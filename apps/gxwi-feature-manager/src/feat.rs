//! Running feat, the only thing that changes a feature. `list --json`
//! answers with one JSON document; `--driven` reports a change as JSON
//! Lines events: progress through the feature's scripts, what they say, and
//! how it ended. feat asks nothing, so nothing is answered. Feature Manager
//! never parses the words feat prints for a person.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use serde::Deserialize;

/// Where feat is.
pub const PROGRAM: &str = "/usr/bin/feat";

/// A feature's place in its lifecycle, as feat says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    #[default]
    NotInstalled,
    Installing,
    Installed,
    Enabling,
    Enabled,
    Disabling,
    Uninstalling,
}

impl State {
    /// A script for it was started and never finished.
    pub fn interrupted(self) -> bool {
        matches!(self, State::Installing | State::Enabling | State::Disabling | State::Uninstalling)
    }
}

/// A feature, as `feat list --json` gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Feature {
    pub name: String,
    pub title: Option<String>,
    /// Paragraphs, separated by a blank line.
    pub description: Option<String>,
    pub state: State,
    /// Its definition is on this machine. A feature whose package was
    /// removed while it was set up has a state and no definition.
    pub defined: bool,
    pub phases: Vec<String>,
    /// The person may change its state: asked of the registry by feat.
    pub may_change: bool,
    /// Why its feature.toml couldn't be read, when it couldn't.
    pub metadata_problem: Option<String>,
}

impl Feature {
    /// What it is called: its title, or its name without one.
    pub fn shown(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.name)
    }
}

/// Every feature on this machine.
pub fn list() -> Result<Vec<Feature>, String> {
    let out = Command::new(PROGRAM)
        .args(["list", "--json"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("feat couldn't be run: {e}"))?;
    if !out.status.success() {
        return Err(said(&String::from_utf8_lossy(&out.stderr)));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("feat's answer couldn't be read: {e}"))
}

/// The last thing feat said on failing, without its name in front.
fn said(stderr: &str) -> String {
    let line = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("feat failed without saying why");
    line.trim().trim_start_matches("feat: ").to_string()
}

/// One event of a driven change.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A lifecycle script is starting: `phase` is install, enable, disable
    /// or uninstall, `step` of `steps`.
    Progress { phase: String, step: usize, steps: usize },
    /// A line a script wrote, or what feat said it did.
    Message { text: String },
    Done {
        #[serde(default)]
        summary: String,
        state: State,
    },
    Error { code: String, message: String },
}

impl Event {
    pub fn ends(&self) -> bool {
        matches!(self, Event::Done { .. } | Event::Error { .. })
    }
}

/// Starts `feat --driven` with `args` and calls `heard` with each event, on
/// a thread of its own, ending with exactly one that ends it. Nothing holds
/// feat to this window: closed, the change still finishes.
pub fn drive(args: &[&str], mut heard: impl FnMut(Event) + Send + 'static) -> Result<(), String> {
    let mut child = Command::new(PROGRAM)
        .arg("--driven")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("feat couldn't be run: {e}"))?;
    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");
    std::thread::spawn(move || {
        let mut ended = false;
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let Ok(event) = serde_json::from_str::<Event>(&line) else { continue };
            ended = event.ends();
            heard(event);
            if ended {
                break;
            }
        }
        // Anything feat wrote outside its events is a fault in it, or a
        // usage error from before it knew it was driven. It is the best
        // word on a run that ended unsaid.
        let mut fault = String::new();
        let _ = std::io::Read::read_to_string(&mut BufReader::new(stderr), &mut fault);
        let _ = child.wait();
        if !ended {
            let message = if fault.trim().is_empty() { "feat stopped without saying how it ended".to_string() } else { said(&fault) };
            heard(Event::Error { code: "failed".into(), message });
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_reads_as_feat_writes_it() {
        let list: Vec<Feature> = serde_json::from_str(
            r#"[{"defined":true,"description":"Keeps it.\n\nAt the next boot.","may_change":true,"name":"dynamic-boot","phases":["install","enable"],"state":"enabled","title":"Dynamic Boot"},
                {"defined":false,"description":null,"may_change":false,"name":"gone","phases":[],"state":"enabling","title":null}]"#,
        )
        .unwrap();
        assert_eq!(list[0].shown(), "Dynamic Boot");
        assert_eq!(list[0].state, State::Enabled);
        assert_eq!(list[1].shown(), "gone");
        assert!(list[1].state.interrupted());
    }

    #[test]
    fn events_read_as_feat_writes_them() {
        let progress: Event = serde_json::from_str(r#"{"event":"progress","phase":"install","step":1,"steps":2}"#).unwrap();
        assert_eq!(progress, Event::Progress { phase: "install".into(), step: 1, steps: 2 });
        let done: Event = serde_json::from_str(r#"{"event":"done","state":"not-installed","summary":"uninstalled x"}"#).unwrap();
        assert!(done.ends());
        let error: Event = serde_json::from_str(r#"{"code":"script","event":"error","message":"feature x: install script failed (exit 1)"}"#).unwrap();
        assert!(error.ends());
        assert!(!Event::Message { text: String::new() }.ends());
    }

    #[test]
    fn a_failure_is_its_last_line() {
        assert_eq!(said("feat: no such feature: x\n"), "no such feature: x");
        assert_eq!(said(""), "feat failed without saying why");
    }
}
