//! Running peipkg, the only thing that changes what is installed. Its
//! queries print one JSON document; its driven mode is a conversation, in
//! JSON Lines, for a change: the plan, each question, progress and how it
//! ended (peipkg TRM, the driven mode). Package Manager never parses the
//! words peipkg prints for a person.

use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde::de::DeserializeOwned;

/// Where peipkg is.
pub const PROGRAM: &str = "/usr/bin/peipkg";

/// Runs a query, `args` with `--json` among them, and reads its answer.
pub fn query<T: DeserializeOwned>(args: &[&str]) -> Result<T, String> {
    let out = Command::new(PROGRAM)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("peipkg couldn't be run: {e}"))?;
    if !out.status.success() {
        return Err(said(&String::from_utf8_lossy(&out.stderr)));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("peipkg's answer couldn't be read: {e}"))
}

/// The last thing peipkg said on failing, without its name in front.
fn said(stderr: &str) -> String {
    let line = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("peipkg failed without saying why");
    line.trim().trim_start_matches("peipkg: ").to_string()
}

/// An installed package, as `list --json` and `info --json` give it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub architecture: String,
    /// The repository it came from; empty for one installed from a file.
    pub origin: String,
    /// Its repository is no longer configured, so nothing updates it.
    pub orphaned: bool,
    pub installed_at: String,
    pub description: String,
    pub size_installed: i64,
    pub license: String,
    pub homepage: String,
    pub alternate_upgrade: String,
    pub dependencies: Vec<String>,
    pub provides: Vec<String>,
}

/// A package a repository offers, as `search --json` gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Offer {
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub repository: String,
    pub description: String,
    pub license: String,
    pub homepage: String,
    pub size_download: i64,
    pub size_installed: i64,
}

/// A configured repository and its trust state, as `repo list --json`
/// gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Repository {
    pub name: String,
    pub base_url: String,
    pub priority: i64,
    pub signature_policy: String,
    pub trust_anchors: Vec<String>,
    pub allow_insecure_transport: bool,
    /// The trust ceremony has run.
    pub trusted: bool,
    pub last_refresh: Option<String>,
    pub index_generated: Option<String>,
    /// Its trust state or its metadata is past its maximum age.
    pub stale: bool,
    pub packages: usize,
}

/// One transaction, as `history --json` gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Transaction {
    pub id: i64,
    /// committed, rolled-back or pending.
    pub state: String,
    pub started_at: String,
    pub summary: String,
    pub operations: Vec<Done>,
}

/// What a transaction did to one package.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Done {
    /// install, upgrade, downgrade, remove or claim.
    pub action: String,
    pub name: String,
    pub from: String,
    pub to: String,
}

/// A role a package can hold, as `claim --json` gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Role {
    pub role: String,
    pub holder: Option<String>,
    pub links: Vec<Link>,
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Link {
    pub path: String,
    pub target: String,
}

/// A named installation root, as `root list --json` gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Root {
    pub name: String,
    pub path: String,
    pub resolved_path: String,
    /// present or dangling.
    pub status: String,
    pub children: Vec<Root>,
}

/// One thing a package owns, as `files --json` gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct File {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub target: String,
}

/// A file that no longer matches its package, as `verify --json` gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Problem {
    pub package: String,
    pub problem: String,
}

/// One event of a driven run.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    Plan(Plan),
    Question(Question),
    Progress(Progress),
    Warning {
        text: String,
    },
    Message {
        text: String,
    },
    Done {
        #[serde(default)]
        summary: Option<String>,
        #[serde(default)]
        transaction: Option<i64>,
    },
    Cancelled {
        #[serde(default)]
        reason: String,
    },
    Error {
        code: String,
        message: String,
    },
}

impl Event {
    /// Whether this is how the run ended.
    pub fn ends(&self) -> bool {
        matches!(self, Event::Done { .. } | Event::Cancelled { .. } | Event::Error { .. })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Plan {
    pub operations: Vec<Operation>,
    pub authorisations: Vec<String>,
    pub notes: Vec<String>,
    pub other_roots: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Operation {
    /// install, upgrade, downgrade or remove.
    pub kind: String,
    pub name: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub repository: Option<String>,
    pub local: bool,
    pub size_download: i64,
    pub size_installed: i64,
    pub root: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Question {
    pub id: u64,
    /// authorise, proceed or modified.
    pub kind: String,
    pub text: String,
    pub package: String,
    pub path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Progress {
    /// fetch, stage, apply, commit or finish.
    pub phase: String,
    pub step: usize,
    pub steps: usize,
    pub package: Option<String>,
}

/// Where a driven run's answers go. Dropping every copy closes peipkg's
/// input, which refuses whatever it asks next.
#[derive(Clone)]
pub struct Answers(Arc<Mutex<Option<ChildStdin>>>);

impl Answers {
    /// Answers question `id`.
    pub fn answer(&self, id: u64, answer: &str) {
        let line = serde_json::json!({ "id": id, "answer": answer }).to_string() + "\n";
        if let Some(stdin) = self.0.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = stdin.write_all(line.as_bytes()).and_then(|()| stdin.flush());
        }
    }

    /// Closes peipkg's input: whatever it asks next is refused.
    pub fn close(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
}

/// Starts `peipkg --driven` with `args`, handing each event to `heard` on
/// a thread of its own, ending with exactly one that [`Event::ends`] it.
pub fn drive(args: &[String], mut heard: impl FnMut(Event) + Send + 'static) -> Result<Answers, String> {
    let mut child = Command::new(PROGRAM)
        .arg("--driven")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("peipkg couldn't be run: {e}"))?;
    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");
    let answers = Answers(Arc::new(Mutex::new(child.stdin.take())));
    let closing = answers.clone();
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
        closing.close();
        // Anything peipkg wrote outside its events is a fault in it: a
        // panic, say. It is the best word on a run that ended unsaid.
        let mut fault = String::new();
        let _ = std::io::Read::read_to_string(&mut BufReader::new(stderr), &mut fault);
        let _ = child.wait();
        if !ended {
            let message = if fault.trim().is_empty() { "peipkg stopped without saying how it ended".to_string() } else { said(&fault) };
            heard(Event::Error { code: "failed".into(), message });
        }
    });
    Ok(answers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_read_as_peipkg_writes_them() {
        let plan: Event = serde_json::from_str(
            r#"{"authorisations":[],"event":"plan","notes":[],"operations":[{"from":"0.1.5-1","kind":"upgrade","local":false,"name":"dev.peios.net","repository":"peios-dev","size_download":212743,"size_installed":655604,"to":"0.1.7-1"}],"other_roots":[]}"#,
        )
        .unwrap();
        let Event::Plan(plan) = plan else { panic!("not a plan") };
        assert_eq!(plan.operations[0].to.as_deref(), Some("0.1.7-1"));
        let asked: Event = serde_json::from_str(r#"{"event":"question","id":2,"kind":"modified","package":"tool","path":"/usr/etc/tool.conf"}"#).unwrap();
        assert!(matches!(asked, Event::Question(Question { id: 2, ref kind, .. }) if kind == "modified"));
        let done: Event = serde_json::from_str(r#"{"event":"done","summary":"2 operations applied","transaction":1}"#).unwrap();
        assert!(done.ends());
        let error: Event = serde_json::from_str(r#"{"code":"stale","event":"error","message":"x"}"#).unwrap();
        assert_eq!(error, Event::Error { code: "stale".into(), message: "x".into() });
        assert!(!Event::Warning { text: String::new() }.ends());
    }

    #[test]
    fn a_failure_is_its_last_line() {
        assert_eq!(said("peipkg: warning: x\npeipkg: info: \"y\" is not installed\n"), "info: \"y\" is not installed");
    }
}
