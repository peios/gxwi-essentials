//! The window: this machine's software, in sections down the side, the one
//! chosen beside them.
//!
//! Everything is read from peipkg as the person, and every change is
//! peipkg's driven run, made as the person: Package Manager has no
//! authority of its own. Whether the person may change anything is asked
//! of the system (`store::may`); where they may not, everything is shown
//! as it is, with the reason said once.

use std::path::PathBuf;
use std::sync::Weak;

use gxwi_file_dialog::{Filter, Mode, Request as Choose};
use libgxwi::settings::{self, Glyph, Nav, Section, Tile};
use libgxwi::{Facts, Fields, Live, Surface, Value};

use crate::packages::{self, Looking};
use crate::peipkg::{self, Event, File, Problem};
use crate::run::{Ended, Run};
use crate::store::{self, Store, Updates};
use crate::{changes, sources, words};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Installed,
    Available,
    Updates,
    History,
    Repositories,
    Roles,
    Roots,
}

impl View {
    const ALL: [(View, &'static str); 7] = [
        (View::Installed, "installed"),
        (View::Available, "available"),
        (View::Updates, "updates"),
        (View::History, "history"),
        (View::Repositories, "repositories"),
        (View::Roles, "roles"),
        (View::Roots, "roots"),
    ];

    fn by(name: &str) -> Option<View> {
        View::ALL.iter().find(|(_, by)| *by == name).map(|(view, _)| *view)
    }

    fn id(self) -> &'static str {
        View::ALL.iter().find(|(view, _)| *view == self).map(|(_, by)| *by).unwrap_or("installed")
    }
}

/// About a repository: a question open under its row, or the form to add
/// one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asking {
    Remove(String),
    /// One set up but never confirmed, to be trusted.
    Trust(String),
    Add,
}

pub struct Manager {
    pub window: Weak<Surface<Manager>>,
    view: View,
    /// The package being looked at, by name.
    package: Option<String>,
    store: Store,
    updates: Updates,
    /// The change being made, or the last one, until it is put away.
    run: Option<Run>,
    runs: u64,
    asking: Option<Asking>,
    /// The files of the package being looked at, once asked for.
    files: Option<(String, Result<Vec<File>, String>)>,
    /// A file dialog is open.
    choosing: bool,
    /// The window was asked to close while a change was being made.
    close_when_done: bool,
    /// The person said to check for updates despite a repository's
    /// out-of-date information.
    stale_ok: bool,
    said: Option<Result<String, String>>,
}

impl Manager {
    pub fn new() -> Manager {
        Manager {
            window: Weak::new(),
            view: View::Installed,
            package: None,
            store: Store::empty(),
            updates: Updates::Unchecked,
            run: None,
            runs: 0,
            asking: None,
            files: None,
            choosing: false,
            close_when_done: false,
            stale_ok: false,
            said: None,
        }
    }

    /// Whether a change is being made, so nothing else may start.
    fn busy(&self) -> bool {
        self.run.as_ref().is_some_and(Run::going)
    }

    /// Reads everything again, on a thread, and shows it when it's read.
    pub fn reread(&self) {
        let window = self.window.clone();
        std::thread::spawn(move || {
            let read = store::read();
            if let Some(window) = window.upgrade() {
                window.update(|manager, fields| manager.heard(read, fields));
            }
        });
    }

    fn heard(&mut self, read: Store, fields: &mut Fields) {
        self.store = read;
        sources::fill_roles(&self.store, fields);
        if let Some(name) = &self.package
            && self.store.package(name).is_none()
            && self.store.offer(name).is_none()
            && self.store.installed.is_ok()
        {
            self.package = None;
        }
    }

    /// Checks for updates, on a thread: a refresh of every trusted
    /// repository first, unless the person may not, then the plan an
    /// update of everything would make, without making it.
    pub fn check(&mut self, anyway: bool) {
        if matches!(self.updates, Updates::Checking) {
            return;
        }
        // Once said, for as long as the window is open: a check changes
        // nothing, and asking again after every change would be noise.
        self.stale_ok |= anyway;
        let anyway = self.stale_ok;
        self.updates = Updates::Checking;
        let window = self.window.clone();
        let may = self.store.may.is_ok();
        std::thread::spawn(move || {
            if may {
                let _ = collect(&refresh_trusted());
            }
            let mut args = vec!["upgrade".to_string(), "--dry-run".to_string()];
            if anyway {
                args.push("--allow-stale".into());
            }
            let found = match collect(&args) {
                Ok((Some(plan), Event::Done { .. })) => Updates::Found(plan.operations),
                Ok((None, Event::Done { .. })) => Updates::Found(Vec::new()),
                Ok((_, Event::Error { code, message })) => Updates::Failed { code, message },
                Ok(_) => Updates::Failed { code: "failed".into(), message: "The check was cancelled".into() },
                Err(why) => Updates::Failed { code: "failed".into(), message: why },
            };
            if let Some(window) = window.upgrade() {
                window.update(|manager, _| manager.updates = found);
            }
        });
    }

    /// Starts a change: peipkg's driven run with `args`. `chosen` is a
    /// change the person picked in a list, whose proceed is theirs already.
    fn start(&mut self, title: String, verb: &'static str, args: Vec<String>, chosen: bool) {
        if self.busy() {
            return;
        }
        self.runs += 1;
        let number = self.runs;
        let mut run = Run::new(number, title, verb, args.clone(), chosen);
        let window = self.window.clone();
        // The window's lock is held here, so the first event waits for the
        // run to be in place before it lands.
        match peipkg::drive(&args, move |event| {
            if let Some(window) = window.upgrade() {
                window.update(|manager, fields| manager.heard_run(number, event, fields));
            }
        }) {
            Ok(answers) => run.answers = Some(answers),
            Err(why) => run.end = Some(Ended::Failed { code: "failed".into(), message: why }),
        }
        self.run = Some(run);
        self.asking = None;
        self.said = None;
    }

    fn heard_run(&mut self, number: u64, event: Event, _fields: &mut Fields) {
        let Some(run) = self.run.as_mut().filter(|r| r.number == number) else { return };
        let ended = event.ends();
        let changed = matches!(event, Event::Done { .. });
        run.heard(event);
        if !ended {
            return;
        }
        if self.close_when_done
            && let Some(window) = self.window.upgrade()
        {
            window.close();
        }
        if changed {
            self.files = None;
            self.reread();
            if matches!(self.updates, Updates::Found(_)) {
                self.updates = Updates::Unchecked;
                self.check(false);
            }
        }
    }

    /// What the change just started says when it is done, having no plan to
    /// say it from.
    fn saying(&mut self, said: &str) {
        if let Some(run) = self.run.as_mut().filter(|r| r.going()) {
            run.said = Some(said.to_string());
        }
    }

    /// Runs again what failed, with the flag that lets it through.
    fn retry(&mut self, flag: &str) {
        let Some(run) = self.run.take() else { return };
        let mut args = run.args.clone();
        self.stale_ok |= flag == "--allow-stale";
        if !flag.is_empty() && !args.iter().any(|a| a == flag) {
            args.push(flag.to_string());
        }
        self.start(run.title, run.verb, args, run.chosen);
        if let Some(said) = run.said {
            self.saying(&said);
        }
    }

    fn answer(&mut self, value: &Value, answer: &str) {
        let id = value.get("id").and_then(Value::as_str).and_then(|id| id.parse().ok()).unwrap_or(0);
        if let Some(run) = &mut self.run {
            run.answer(id, answer);
        }
    }

    /// Opens the file dialog for a package file to install.
    fn choose_file(&mut self) {
        if self.choosing {
            return;
        }
        let request = Choose {
            mode: Mode::Open,
            title: "Install from a file".into(),
            purpose: Some("A package file to install. It isn't from a repository, so nothing vouches for it: install only what you trust.".into()),
            folder: None,
            name: None,
            filters: vec![Filter { name: "Packages".into(), extensions: vec!["peipkg".into()] }],
        };
        let window = self.window.clone();
        let opened = gxwi_file_dialog::choose(&request, move |file: Option<PathBuf>| {
            let Some(window) = window.upgrade() else { return };
            window.update(|manager, _| {
                manager.choosing = false;
                if let Some(file) = file {
                    let shown = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    manager.start(format!("Install {shown}"), "Install", vec!["install".into(), file.display().to_string()], false);
                }
            });
        });
        match opened {
            Ok(()) => self.choosing = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.said = Some(Err("The file dialog isn't installed.".into())),
            Err(e) => self.said = Some(Err(format!("The file dialog couldn't be opened: {e}."))),
        }
    }

    /// Checks a package's files against what was installed.
    fn verify(&mut self, name: &str) {
        let short = words::short(name);
        self.said = Some(match peipkg::query::<Vec<Problem>>(&["verify", "--json", name]) {
            Ok(problems) if problems.is_empty() => Ok(format!("Every file of {short} is as it was installed.")),
            Ok(problems) => Err(format!(
                "{} of {short} {} changed: {}.",
                words::count(problems.len(), "file", "files"),
                if problems.len() == 1 { "has" } else { "have" },
                problems.iter().map(|p| p.problem.as_str()).collect::<Vec<_>>().join("; ")
            )),
            Err(why) => Err(words::sentence(&why)),
        });
    }

    /// Adds the repository the form describes.
    fn add_repo(&mut self, fields: &mut Fields) {
        let name = fields.get("repo-name").trim().to_string();
        let url = fields.get("repo-url").trim().to_string();
        let key: String = fields.get("repo-key").chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
        let problem = if name.is_empty() || name.contains(['/', ' ']) {
            Some("Give it a name, without spaces or slashes.")
        } else if self.store.repositories.as_ref().is_ok_and(|r| r.iter().any(|r| r.name == name)) {
            Some("A repository has that name already.")
        } else if !(url.starts_with("https://") || url.starts_with("file:///")) {
            Some("The address must start with https:// or file:///.")
        } else if key.len() != 64 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
            Some("The fingerprint is 64 hexadecimal digits.")
        } else {
            None
        };
        if let Some(problem) = problem {
            self.said = Some(Err(problem.into()));
            return;
        }
        for field in ["repo-name", "repo-url", "repo-key"] {
            fields.set(field, "");
        }
        self.start(format!("Add {name}"), "Add", vec!["repo".into(), "add".into(), name.clone(), url, "--anchor".into(), key], true);
        self.saying(&format!("{name} added and trusted. What it offers is under Available."));
    }

    fn nav(&self) -> Vec<Nav> {
        // Where nothing can be read, every section says so, the same way.
        let hidden = self.store.look.is_err();
        let section = |view: View, title, now: String, glyph, tile| {
            let now = if hidden { "Not readable".to_string() } else { now };
            Nav::Section(Section { id: view.id(), title, now, glyph, tile })
        };
        let or = |read: bool, now: String| if read { now } else { "Unavailable".to_string() };
        let installed = or(self.store.installed.is_ok(), words::count(self.store.installed.as_ref().map_or(0, Vec::len), "package", "packages"));
        let available = or(self.store.offers.is_ok(), format!("{} to install", self.store.not_installed().len()));
        let updates = match &self.updates {
            Updates::Unchecked | Updates::Checking => "Checking…".to_string(),
            Updates::Found(ops) if ops.is_empty() => "Up to date".to_string(),
            Updates::Found(ops) => words::count(ops.len(), "update", "updates"),
            Updates::Failed { .. } => "Not checked".to_string(),
        };
        let history = match &self.store.history {
            Ok(_) if self.store.pending() => "Interrupted".to_string(),
            Ok(list) if list.is_empty() => "No changes yet".to_string(),
            Ok(list) => words::count(list.len(), "change", "changes"),
            Err(_) => "Unavailable".to_string(),
        };
        let repositories = match &self.store.repositories {
            Ok(list) if list.iter().any(|r| r.stale) => "Out of date".to_string(),
            Ok(list) => words::count(list.len(), "repository", "repositories"),
            Err(_) => "Unavailable".to_string(),
        };
        let roles = or(self.store.roles.is_ok(), words::count(self.store.roles.as_ref().map_or(0, Vec::len), "role", "roles"));
        let roots = match &self.store.roots {
            Ok(list) if list.is_empty() => "None".to_string(),
            Ok(list) => words::count(list.len(), "root", "roots"),
            Err(_) => "Unavailable".to_string(),
        };
        vec![
            Nav::Heading("Software"),
            section(View::Installed, "Installed", installed, Glyph::Package, Tile::Blue),
            section(View::Available, "Available", available, Glyph::Download, Tile::Teal),
            section(View::Updates, "Updates", updates, Glyph::Refresh, Tile::Green),
            Nav::Heading("This Machine"),
            section(View::History, "History", history, Glyph::Clock, Tile::Slate),
            section(View::Repositories, "Repositories", repositories, Glyph::Server, Tile::Violet),
            section(View::Roles, "Roles", roles, Glyph::Tag, Tile::Orange),
            section(View::Roots, "Installation Roots", roots, Glyph::Folder, Tile::Pink),
        ]
    }
}

/// The refresh of every trusted repository, by name. `peipkg refresh` with
/// no names also runs the trust ceremony for a repository set up but never
/// confirmed, and trusting one is the person's choice, under Repositories,
/// never a side effect of looking for updates.
fn refresh_trusted() -> Vec<String> {
    let trusted = peipkg::query::<Vec<peipkg::Repository>>(&["repo", "list", "--json"]).unwrap_or_default();
    let mut args = vec!["refresh".to_string()];
    args.extend(trusted.into_iter().filter(|r| r.trusted).map(|r| r.name));
    args
}

/// Runs peipkg's driven mode to its end with nothing to answer, and gives
/// the plan, if it made one, and how it ended.
fn collect(args: &[String]) -> Result<(Option<peipkg::Plan>, Event), String> {
    if args == ["refresh"] {
        // Nothing is trusted: nothing to refresh.
        return Ok((None, Event::Done { summary: None, transaction: None }));
    }
    let (send, receive) = std::sync::mpsc::channel();
    let answers = peipkg::drive(args, move |event| {
        let _ = send.send(event);
    })?;
    answers.close();
    let mut plan = None;
    for event in receive {
        match event {
            Event::Plan(p) => plan = Some(p),
            end if end.ends() => return Ok((plan, end)),
            _ => {}
        }
    }
    Err("peipkg stopped without saying how it ended".into())
}

impl Live for Manager {
    fn render(&self, facts: &Facts) -> String {
        let fields = facts.fields;
        let busy = self.busy();
        let now = jiff::Timestamp::now().as_second();
        let page = if let Some(run) = &self.run {
            let mut page = crate::run::page(run);
            if self.close_when_done && run.going() {
                page = settings::caution("Package Manager closes when these changes are finished.") + &page;
            }
            page
        } else if let Some(name) = &self.package {
            let files = self.files.as_ref().filter(|(of, _)| of == name).map(|(_, files)| files);
            let from = if self.view == View::Installed { "Installed" } else if self.view == View::Updates { "Updates" } else { "Available" };
            packages::detail(name, &self.store, &Looking { updates: &self.updates, files, from, busy })
        } else {
            match self.view {
                View::Installed => packages::installed(&self.store, fields, &self.updates),
                View::Available => packages::available(&self.store, fields, self.choosing),
                View::Updates => changes::updates(&self.store, &self.updates, busy),
                View::History => changes::history(&self.store, busy),
                View::Repositories => sources::repositories(&self.store, self.asking.as_ref(), busy, now),
                View::Roles => sources::roles(&self.store, busy),
                View::Roots => sources::roots(&self.store),
            }
        };
        settings::window(&self.nav(), self.view.id(), &page, &settings::status(self.said.as_ref(), ""))
    }

    fn input(&mut self, name: &str, fields: &mut Fields) {
        let Some(role) = name.strip_prefix("role:") else { return };
        let holder = fields.get(name).to_string();
        if self.busy() || self.store.may.is_err() {
            sources::fill_roles(&self.store, fields);
            return;
        }
        let (title, args, said) = if holder.is_empty() {
            (format!("Stop providing {role}"), vec!["claim".into(), role.to_string(), "revoke".into()], format!("Nothing provides {role} now."))
        } else {
            let short = words::short(&holder).to_string();
            (format!("Provide {role} with {short}"), vec!["claim".into(), role.to_string(), "grant".into(), holder], format!("{short} provides {role} now."))
        };
        self.start(title, "Change", args, true);
        self.saying(&said);
    }

    fn event(&mut self, name: &str, value: &Value, fields: &mut Fields) {
        let named = || value.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        match name {
            "section" => {
                if let Some(view) = value.get("section").and_then(Value::as_str).and_then(View::by) {
                    // Read again: peipkg may have been run from a terminal
                    // since, and nothing tells this window when.
                    self.reread();
                    self.view = view;
                    self.package = None;
                    self.asking = None;
                    self.said = None;
                    if self.run.as_ref().is_some_and(|r| !r.going()) {
                        self.run = None;
                    }
                }
            }
            "open" => {
                self.package = Some(named());
                self.said = None;
            }
            "back" => {
                self.package = None;
                self.said = None;
            }
            "install" => {
                let n = named();
                self.start(format!("Install {}", words::short(&n)), "Install", vec!["install".into(), n], false);
            }
            "update" => {
                let n = named();
                self.start(format!("Update {}", words::short(&n)), "Update", vec!["upgrade".into(), n], false);
            }
            "update-all" => self.start("Update Everything".into(), "Update", vec!["upgrade".into()], false),
            "remove" => {
                let n = named();
                self.start(format!("Remove {}", words::short(&n)), "Remove", vec!["uninstall".into(), n], false);
            }
            "undo" => self.start("Undo the Last Change".into(), "Undo", vec!["undo".into()], false),
            "recover" => self.start("Resolve the Interrupted Change".into(), "Resolve", vec!["recover".into()], true),
            "refresh" => {
                let mut args = vec!["refresh".to_string()];
                args.extend(self.store.repositories.iter().flatten().filter(|r| r.trusted).map(|r| r.name.clone()));
                if args.len() > 1 {
                    self.start("Refresh Repositories".into(), "Refresh", args, true);
                    self.saying("Every trusted repository is up to date with what it offers.");
                }
            }
            "install-file" => self.choose_file(),
            "check" => self.check(false),
            "check-anyway" => {
                self.updates = Updates::Unchecked;
                self.check(true);
            }
            "verify" => self.verify(&named()),
            "files" => {
                let n = named();
                let files = peipkg::query::<Vec<File>>(&["files", "--json", &n]);
                self.files = Some((n, files));
            }
            "ask-add-repo" => self.asking = Some(Asking::Add),
            "ask-remove-repo" => {
                let repo = value.get("repo").and_then(Value::as_str).unwrap_or_default().to_string();
                self.asking = Some(Asking::Remove(repo));
            }
            "ask-trust-repo" => {
                let repo = value.get("repo").and_then(Value::as_str).unwrap_or_default().to_string();
                self.asking = Some(Asking::Trust(repo));
            }
            "trust-repo" => {
                // The configured form of `repo add`: the trust ceremony, with
                // the key already given in its .repo file.
                let repo = value.get("repo").and_then(Value::as_str).unwrap_or_default().to_string();
                self.start(format!("Trust {repo}"), "Trust", vec!["repo".into(), "add".into(), repo.clone()], true);
                self.saying(&format!("{repo} is trusted, and what it offers can be installed."));
            }
            "cancel" => {
                self.asking = None;
                self.said = None;
            }
            "add-repo" => self.add_repo(fields),
            "remove-repo" => {
                let repo = value.get("repo").and_then(Value::as_str).unwrap_or_default().to_string();
                self.start(format!("Remove {repo}"), "Remove", vec!["repo".into(), "remove".into(), repo.clone()], true);
                self.saying(&format!("{repo} removed. What was installed from it stays installed."));
            }
            "allow" | "proceed" => self.answer(value, "yes"),
            "refuse" => self.answer(value, "no"),
            "modified" => {
                let answer = value.get("answer").and_then(Value::as_str).unwrap_or("abort").to_string();
                self.answer(value, &answer);
            }
            "retry" => {
                let flag = value.get("flag").and_then(Value::as_str).unwrap_or_default().to_string();
                self.retry(&flag);
            }
            "notes" => {
                if let Some(run) = &mut self.run {
                    run.notes_open = true;
                }
            }
            "dismiss" if !self.busy() => self.run = None,
            _ => {}
        }
    }

    fn closing(&mut self, _fields: &mut Fields) -> bool {
        // A change past its approval finishes whatever happens; the window
        // stays to show how it ended, then goes.
        if self.run.as_ref().is_some_and(Run::committed) {
            self.close_when_done = true;
            return false;
        }
        if let Some(answers) = self.run.as_ref().and_then(|r| r.answers.as_ref()) {
            answers.close();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_section_names_a_view() {
        for (view, by) in View::ALL {
            assert_eq!(View::by(by), Some(view));
            assert_eq!(view.id(), by);
        }
        assert_eq!(View::by("nonsense"), None);
    }
}
