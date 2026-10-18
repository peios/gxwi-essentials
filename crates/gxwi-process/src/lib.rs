//! Driven JSON-line child processes shared by the management applications.
//!
//! The caller defines terminal events and their fallback error wording. Both
//! output pipes are drained concurrently, and the worker owns and reaps the
//! child. Closing a window does not kill an operation: its owner closes
//! [`Answers`] explicitly when unanswered questions should be refused.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use serde::de::DeserializeOwned;

/// A driven child's answer stream. The reader keeps a copy until it finishes,
/// so callers must use [`Self::close`] to refuse further questions explicitly.
#[derive(Clone)]
pub struct Answers(Arc<Mutex<Option<ChildStdin>>>);

impl Answers {
    /// Write one JSON answer, or report that the child can no longer receive it.
    pub fn answer(&self, id: u64, answer: &str) -> io::Result<()> {
        let line = serde_json::json!({ "id": id, "answer": answer }).to_string() + "\n";
        let mut input = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let stdin = input.as_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "the answer stream is closed")
        })?;
        stdin.write_all(line.as_bytes())?;
        stdin.flush()
    }

    /// Close standard input, refusing whatever the child asks next.
    pub fn close(&self) {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }
}

/// Own a child until it has been reaped, including worker-startup failures.
struct Running(Option<Child>);

impl Running {
    fn wait(&mut self) -> io::Result<ExitStatus> {
        let status = self.0.as_mut().expect("an unreaped child").wait()?;
        self.0.take();
        Ok(status)
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            // Cleanup after a startup error or a panicking callback. Kill may
            // fail because the child already exited; wait still reaps it.
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Start a driven command with the caller's program, arguments and input mode.
///
/// Events are parsed as `E`; malformed lines are ignored, as in the app
/// protocols. The first terminal event ends delivery. If none arrives, `failed`
/// receives the child's diagnostic output and creates exactly one terminal
/// fallback. The worker closes input and reaps the child in either case.
/// No output-size or execution-time policy is imposed here.
///
/// # Errors
/// Returns an error if the child or its owning worker cannot be started.
/// A failure to start the stderr reader is delivered through `failed`.
pub fn drive<E>(
    mut command: Command,
    ends: fn(&E) -> bool,
    failed: impl FnOnce(&str) -> E + Send + 'static,
    heard: impl FnMut(E) + Send + 'static,
) -> io::Result<Answers>
where
    E: DeserializeOwned + Send + 'static,
{
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = command.spawn()?;
    start(child, ends, failed, heard).map(|(answers, _worker)| answers)
}

fn start<E>(
    mut child: Child,
    ends: fn(&E) -> bool,
    failed: impl FnOnce(&str) -> E + Send + 'static,
    mut heard: impl FnMut(E) + Send + 'static,
) -> io::Result<(Answers, JoinHandle<()>)>
where
    E: DeserializeOwned + Send + 'static,
{
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let answers = Answers(Arc::new(Mutex::new(child.stdin.take())));
    let closing = answers.clone();
    let mut running = Running(Some(child));
    let worker = thread::Builder::new()
        .name("gxwi-driven-process".into())
        .spawn(move || {
            let reader = thread::Builder::new()
                .name("gxwi-process-stderr".into())
                .spawn(move || {
                    let mut fault = String::new();
                    // Preserve any diagnostic prefix if the stream later becomes
                    // unreadable. EOF without a terminal event still gets a fallback.
                    let _ = BufReader::new(stderr).read_to_string(&mut fault);
                    fault
                });
            let reader = match reader {
                Ok(reader) => reader,
                Err(error) => {
                    closing.close();
                    drop(running);
                    heard(failed(&format!(
                        "The diagnostic reader couldn't be started: {error}"
                    )));
                    return;
                }
            };
            let mut ended = false;
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(event) = serde_json::from_str::<E>(&line) else {
                    continue;
                };
                ended = ends(&event);
                heard(event);
                if ended {
                    break;
                }
            }
            closing.close();
            let fault = reader
                .join()
                .unwrap_or_else(|_| "The diagnostic reader stopped unexpectedly".into());
            // A terminal protocol event remains authoritative. Otherwise the
            // diagnostic or the caller's generic fallback explains the failure.
            let _ = running.wait();
            if !ended {
                heard(failed(&fault));
            }
        })?;
    Ok((answers, worker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    #[serde(tag = "event", rename_all = "snake_case")]
    enum Event {
        Message { text: String },
        Done,
        Error { message: String },
    }

    fn ends(event: &Event) -> bool {
        !matches!(event, Event::Message { .. })
    }

    fn failed(message: &str) -> Event {
        Event::Error {
            message: message.to_string(),
        }
    }

    /// Run potentially blocking regressions in a disposable test process.
    /// A broken drain order fails within ten seconds instead of hanging CI.
    fn fixture(mode: &str) {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "tests::process_fixture", "--nocapture"])
            .env("GXWI_PROCESS_TEST", mode)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if child.try_wait().unwrap().is_some() {
                let output = child.wait_with_output().unwrap();
                assert!(
                    output.status.success(),
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                return;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "process fixture {mode} timed out: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn stderr_larger_than_a_pipe_does_not_block_terminal_events() {
        fixture("large-stderr");
    }

    #[test]
    fn eof_without_a_terminal_event_reports_the_diagnostic_once() {
        fixture("eof");
    }

    #[test]
    fn invalid_lines_are_skipped_and_only_one_terminal_event_is_delivered() {
        fixture("events");
    }

    #[test]
    fn answers_reach_the_child_and_close_refuses_more_questions() {
        fixture("answers");
        fixture("close");
    }

    #[test]
    fn spawn_failure_is_returned_without_an_event() {
        let error = drive(
            Command::new("/no/such/gxwi-process-test"),
            ends,
            failed,
            |_| panic!("no child was started"),
        );
        assert_eq!(error.err().unwrap().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn process_fixture() {
        let Ok(mode) = std::env::var("GXWI_PROCESS_TEST") else {
            return;
        };
        let script = match mode.as_str() {
            "large-stderr" => {
                "i=0; while [ \"$i\" -lt 20000 ]; do printf 'diagnostic output before the terminal event\\n' >&2; i=$((i+1)); done; printf '{\"event\":\"done\"}\\n'"
            }
            "eof" => "printf 'last diagnostic' >&2",
            "events" => {
                "printf 'not json\\n{\"event\":\"message\",\"text\":\"hello\"}\\n{\"event\":\"done\"}\\n{\"event\":\"done\"}\\n'"
            }
            "answers" => {
                "IFS= read -r answer; [ \"$answer\" = '{\"answer\":\"yes\",\"id\":7}' ] || exit 1; printf '{\"event\":\"done\"}\\n'"
            }
            "close" => "if IFS= read -r answer; then exit 1; fi; printf '{\"event\":\"done\"}\\n'",
            _ => panic!("unknown fixture {mode}"),
        };
        let child = Command::new("/bin/sh")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id();
        let (send, receive) = std::sync::mpsc::channel();
        let (answers, worker) = start(child, ends, failed, move |event| {
            send.send(event).unwrap();
        })
        .unwrap();
        if mode == "answers" {
            answers.answer(7, "yes").unwrap();
        } else if mode == "close" {
            answers.close();
        }
        worker.join().unwrap();
        let events: Vec<Event> = receive.iter().collect();
        let expected = match mode.as_str() {
            "eof" => vec![failed("last diagnostic")],
            "events" => vec![
                Event::Message {
                    text: "hello".into(),
                },
                Event::Done,
            ],
            _ => vec![Event::Done],
        };
        assert_eq!(events, expected);
        assert_eq!(
            answers.answer(8, "no").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "the child was not reaped"
        );
    }
}
