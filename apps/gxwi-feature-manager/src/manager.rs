//! The window: every feature on this machine, what it is and how it is set
//! up, with the changes that apply to it.
//!
//! Everything is read from feat as the person, and every change is feat's
//! driven run, made as the person: Feature Manager has no authority of its
//! own. Whether the person may change a feature is asked of the system, by
//! feat; where they may not, the feature is shown as it is, with the reason.

use std::sync::Weak;

use libgxwi::settings;
use libgxwi::{Facts, Fields, Live, Surface, Value};

use crate::feat::{self, Event, Feature};
use crate::page;

/// A change feat makes, by the verb it is run with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// Install, then turn on.
    Add,
    Install,
    Enable,
    Disable,
    /// Turn off if on, then uninstall.
    Remove,
}

impl Verb {
    pub const ALL: [(Verb, &'static str); 5] =
        [(Verb::Add, "add"), (Verb::Install, "install"), (Verb::Enable, "enable"), (Verb::Disable, "disable"), (Verb::Remove, "remove")];

    pub fn by(name: &str) -> Option<Verb> {
        Verb::ALL.iter().find(|(_, by)| *by == name).map(|(verb, _)| *verb)
    }

    pub fn arg(self) -> &'static str {
        Verb::ALL.iter().find(|(verb, _)| *verb == self).map(|(_, by)| *by).unwrap_or("install")
    }

    /// What it is doing, as the run's title: "Turning On".
    pub fn doing(self) -> &'static str {
        match self {
            Verb::Add => "Installing and Turning On",
            Verb::Install => "Installing",
            Verb::Enable => "Turning On",
            Verb::Disable => "Turning Off",
            Verb::Remove => "Removing",
        }
    }

    /// What it did, said of a feature: "Dynamic Boot is on."
    pub fn done(self, shown: &str) -> String {
        match self {
            Verb::Add => format!("{shown} is installed and on."),
            Verb::Install => format!("{shown} is installed."),
            Verb::Enable => format!("{shown} is on."),
            Verb::Disable => format!("{shown} is off."),
            Verb::Remove => format!("{shown} is removed."),
        }
    }
}

/// The features, as far as they have been read.
pub enum Listing {
    Reading,
    Listed(Vec<Feature>),
    Failed(String),
}

/// How a run ended, when it ended badly: feat's code and words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: String,
    pub message: String,
}

/// A change being made, or one that failed until it is put away.
pub struct Run {
    /// Which run this is, so a run's late events can't land on the next.
    pub number: u64,
    /// The feature, by name.
    pub name: String,
    /// The feature, as it is shown.
    pub shown: String,
    pub verb: Verb,
    /// The script running now, and where it falls: (phase, step, steps).
    pub progress: Option<(String, usize, usize)>,
    /// Everything feat and the scripts said, in order.
    pub output: Vec<String>,
    pub failed: Option<Failure>,
}

impl Run {
    pub fn going(&self) -> bool {
        self.failed.is_none()
    }
}

pub struct Manager {
    pub window: Weak<Surface<Manager>>,
    pub features: Listing,
    pub run: Option<Run>,
    runs: u64,
    /// The feature whose removal is being asked about, by name.
    pub asking: Option<String>,
    /// The window was asked to close while a change was being made.
    pub close_when_done: bool,
    said: Option<Result<String, String>>,
}

impl Manager {
    pub fn new() -> Manager {
        Manager { window: Weak::new(), features: Listing::Reading, run: None, runs: 0, asking: None, close_when_done: false, said: None }
    }

    /// Whether a change is being made, so nothing else may start.
    pub fn busy(&self) -> bool {
        self.run.as_ref().is_some_and(Run::going)
    }

    /// Reads every feature again, on a thread, and shows them when read.
    pub fn reread(&self) {
        let window = self.window.clone();
        std::thread::spawn(move || {
            let read = match feat::list() {
                Ok(features) => Listing::Listed(features),
                Err(why) => Listing::Failed(why),
            };
            if let Some(window) = window.upgrade() {
                window.update(|manager, _| manager.features = read);
            }
        });
    }

    fn feature(&self, name: &str) -> Option<&Feature> {
        match &self.features {
            Listing::Listed(features) => features.iter().find(|f| f.name == name),
            _ => None,
        }
    }

    /// Starts `verb` on the feature `name`.
    fn start(&mut self, verb: Verb, name: &str) {
        if self.busy() {
            return;
        }
        let Some(feature) = self.feature(name).filter(|f| f.may_change && f.defined) else { return };
        let shown = feature.shown().to_string();
        self.runs += 1;
        let number = self.runs;
        let window = self.window.clone();
        let mut run = Run { number, name: name.to_string(), shown, verb, progress: None, output: Vec::new(), failed: None };
        // The window's lock is held here, so the first event waits for the
        // run to be in place before it lands.
        if let Err(why) = feat::drive(&[verb.arg(), name], move |event| {
            if let Some(window) = window.upgrade() {
                window.update(|manager, _| manager.heard(number, event));
            }
        }) {
            run.failed = Some(Failure { code: "failed".into(), message: why });
        }
        self.run = Some(run);
        self.asking = None;
        self.said = None;
    }

    fn heard(&mut self, number: u64, event: Event) {
        let Some(run) = self.run.as_mut().filter(|r| r.number == number) else { return };
        match event {
            Event::Progress { phase, step, steps } => run.progress = Some((phase, step, steps)),
            Event::Message { text } => run.output.push(text),
            // Said the same whether its scripts ran or it was already so,
            // since either way it is now.
            Event::Done { .. } => {
                self.said = Some(Ok(run.verb.done(&run.shown)));
                self.run = None;
            }
            Event::Error { code, message } => run.failed = Some(Failure { code, message }),
        }
        if self.run.as_ref().is_some_and(Run::going) {
            return;
        }
        // It ended: whatever happened, what feat holds now may differ.
        self.reread();
        if self.close_when_done
            && let Some(window) = self.window.upgrade()
        {
            window.close();
        }
    }
}

impl Live for Manager {
    fn render(&self, _facts: &Facts) -> String {
        settings::single(&page::page(self), &settings::status(self.said.as_ref(), ""))
    }

    fn event(&mut self, name: &str, value: &Value, _fields: &mut Fields) {
        let named = || value.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        match name {
            "change" => {
                if let Some(verb) = value.get("verb").and_then(Value::as_str).and_then(Verb::by) {
                    self.start(verb, &named());
                }
            }
            "ask-remove" if !self.busy() => {
                self.asking = Some(named());
                self.said = None;
            }
            "cancel" => self.asking = None,
            "dismiss" if !self.busy() => self.run = None,
            _ => {}
        }
    }

    fn closing(&mut self, _fields: &mut Fields) -> bool {
        // A change finishes whatever happens; the window stays to show how
        // it ended, then goes.
        if self.busy() {
            self.close_when_done = true;
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_verb_is_feats() {
        for (verb, by) in Verb::ALL {
            assert_eq!(Verb::by(by), Some(verb));
            assert_eq!(verb.arg(), by);
        }
        assert_eq!(Verb::by("uninstall"), None);
    }
}
