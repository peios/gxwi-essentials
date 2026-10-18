//! The window: every service defined, what each is doing, and the commands
//! that start, stop, restart, reload and reset them.
//!
//! WHAT IS LISTED is what peinit lists, which is every service the person may
//! query, and with it the services defined in the registry that peinit left
//! out: their definitions are readable to the person, so that they exist is
//! already theirs to know, but their state is not. Where the registry cannot
//! be read either, the window says that some may not be listed at all.
//!
//! WHAT THE PERSON MAY DO is asked of the system, never guessed: AccessCheck
//! against the descriptor peinit itself checks a command against. A command
//! is offered only where the person has its right and the service is in a
//! state the command does something in, by peinit's own admission matrix;
//! otherwise its button is there, unpressable, and the details say why.
//! Where who may control a service cannot be read, the commands are offered
//! and peinit decides, and the details say that too.
//!
//! A COMMAND is asked for and not waited on: the window follows the
//! operation it started and says what came of it, and carries on being a
//! window meanwhile. One command at a time per service.
//!
//! PERMISSIONS, who may control a service and who may change its
//! definition, open in gxwi-sd-editor, a dialog of its own. The window reads
//! the descriptor, says whether the person can change it, and applies what
//! comes back (`permissions`); then it looks again, since what the person
//! may do may have changed with it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Weak};

use gxwi_sd_editor::Part;
use libgxwi::{Facts, Fields, Live, Surface, Value, escape};
use peinit::client::{Admission, Command, Failure, State, Status, Summary, Timer, admission};

use crate::permissions::{self, Which};
use crate::system::{self, Defined, From, Hidden, Outcome, Rights, Seen, Unasked};
use crate::words;

/// What is said where peinit's control socket turns the person away.
const REFUSED: &str = "The service manager does not let you ask it anything, so the state of the services here is not yours to see, and none of them is yours to start or stop.";

/// What shows a service's logs.
const EVENT_VIEWER: &str = "/usr/bin/gxwi-event-viewer";

/// The commands, in the order their buttons are drawn.
const COMMANDS: [Command; 5] = [
    Command::Start,
    Command::Stop,
    Command::Restart,
    Command::Reload,
    Command::Reset,
];

/// What the window last said about a command, or about anything else.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Said {
    text: String,
    bad: bool,
}

/// A service's state in full, or why it is not to be had.
struct Detail {
    service: String,
    status: Result<Status, String>,
}

/// Permissions to take away, so that the default is what applies: a
/// service's own, or the default set for every service.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Forget {
    Service(String),
    Every,
}

/// A row of the listing.
enum Row<'a> {
    Listed(&'a Summary),
    /// Defined, with its state not the person's to see.
    Hidden(&'a Defined),
}

impl Row<'_> {
    fn service(&self) -> &str {
        match self {
            Row::Listed(summary) => &summary.service,
            Row::Hidden(defined) => &defined.name,
        }
    }

    fn display_name(&self) -> Option<&str> {
        match self {
            Row::Listed(summary) => summary.display_name.as_deref(),
            Row::Hidden(defined) => defined.display_name.as_deref(),
        }
    }

    fn description(&self) -> Option<&str> {
        match self {
            Row::Listed(summary) => summary.description.as_deref(),
            Row::Hidden(defined) => defined.description.as_deref(),
        }
    }

    /// What it is called: its display name if it has one.
    fn title(&self) -> &str {
        self.display_name().unwrap_or(self.service())
    }

    fn state(&self) -> Option<State> {
        match self {
            Row::Listed(summary) => Some(summary.state),
            Row::Hidden(_) => None,
        }
    }
}

pub struct Manager {
    services: Vec<Summary>,
    hidden: Hidden,
    rights: HashMap<String, Rights>,
    from: HashMap<String, From>,
    /// Where who may control a service without its own comes from.
    every: Result<From, String>,
    /// Whether that may be changed, or why not.
    every_changeable: Result<(), String>,
    /// Whether the definition of the service picked may be changed, or why
    /// not, as last looked at.
    changeable: Option<(String, Result<(), String>)>,
    /// Whether a service may be defined, or why not.
    creatable: Result<(), String>,
    /// What is being asked of the person before it is done.
    asking: Option<Forget>,
    /// The permissions open in the editor, by service and which.
    editing: HashSet<(String, Which)>,
    /// The services whose definitions are open, each in a window of its
    /// own, by name in lower case.
    defining: HashSet<String>,
    /// Whether peinit has been asked yet.
    looked: bool,
    /// Whether peinit's control socket turned the person away: then nothing
    /// is theirs to ask it, state or command.
    refused: bool,
    /// Why peinit could not be asked, the last time it was.
    trouble: Option<String>,
    picked: Option<String>,
    detail: Option<Detail>,
    /// What is under way, by service.
    working: BTreeMap<String, String>,
    said: Option<Said>,
    /// What a timer's times are said against.
    clock: words::Clock,
    /// The window, for work done aside to say when it is done. Without one,
    /// nothing is asked of the system.
    pub window: Weak<Surface<Manager>>,
}

impl Manager {
    pub fn new() -> Manager {
        Manager {
            services: Vec::new(),
            hidden: Hidden::These(Vec::new()),
            rights: HashMap::new(),
            from: HashMap::new(),
            every: Err("it has not been read yet".into()),
            every_changeable: Err("it has not been read yet".into()),
            changeable: None,
            creatable: Err("it has not been asked yet".into()),
            asking: None,
            editing: HashSet::new(),
            defining: HashSet::new(),
            looked: false,
            refused: false,
            trouble: None,
            picked: None,
            detail: None,
            working: BTreeMap::new(),
            said: None,
            clock: words::Clock::Machine,
            window: Weak::new(),
        }
    }

    /// The service picked, for whoever looks again to ask about, if its state
    /// is the person's to ask for. One whose state is hidden is not asked
    /// about: peinit records every refusal, and a window left open on one
    /// would be refused every time it looked.
    pub fn picked(&self) -> Option<String> {
        self.picked.clone().filter(|picked| {
            self.services
                .iter()
                .any(|service| &service.service == picked)
        })
    }

    /// What a look at the services found.
    pub fn seen(&mut self, seen: Seen) {
        self.looked = true;
        self.refused = seen.services == Err(Unasked::NotAllowed);
        match seen.services {
            Ok(services) => {
                self.services = services;
                self.trouble = None;
            }
            Err(Unasked::NotAllowed) => {
                self.services = Vec::new();
                self.trouble = Some(REFUSED.into());
            }
            // What was listed stays, under why it is not current.
            Err(Unasked::Unreachable(why)) => self.trouble = Some(why),
        }
        self.hidden = seen.hidden;
        self.rights = seen.rights;
        self.from = seen.from;
        self.every = seen.every;
        self.every_changeable = seen.every_changeable;
        self.creatable = seen.creatable;
        if seen.changeable.is_some() {
            self.changeable = seen.changeable;
        }
    }

    /// The service picked, whatever is known of it: what a look asks the
    /// registry about.
    pub fn chosen(&self) -> Option<String> {
        self.picked.clone()
    }

    /// What `service` is doing in full, or why that is not to be had.
    pub fn detailed(&mut self, service: &str, status: Result<Status, Failure>) {
        if self.picked.as_deref() != Some(service) {
            return;
        }
        // Turned away, the person is told so once, and not again for each
        // service.
        if self.refused {
            self.detail = None;
            return;
        }
        let status = status.map_err(|failure| match &failure {
            Failure::Refused { code, .. } if code == "ACCESS_DENIED" => {
                "Its state is not yours to see.".to_string()
            }
            Failure::Refused { code, .. } if code == "UNKNOWN_SERVICE" => {
                "The service manager does not know it: its definition may not have been read yet."
                    .to_string()
            }
            failure => system::unreachable(failure),
        });
        self.detail = Some(Detail {
            service: service.to_string(),
            status,
        });
    }

    /// The rows, as the find field narrows them, by title.
    fn rows(&self, find: &str) -> Vec<Row<'_>> {
        let find = find.trim().to_lowercase();
        let hidden: &[Defined] = match &self.hidden {
            Hidden::These(defined) => defined,
            Hidden::Unknowable => &[],
        };
        let mut rows: Vec<Row> = self
            .services
            .iter()
            .map(Row::Listed)
            .chain(hidden.iter().map(Row::Hidden))
            .collect();
        rows.retain(|row| {
            find.is_empty()
                || row.service().to_lowercase().contains(&find)
                || row.title().to_lowercase().contains(&find)
                || row
                    .description()
                    .is_some_and(|description| description.to_lowercase().contains(&find))
        });
        rows.sort_by_cached_key(|row| (row.title().to_lowercase(), row.service().to_string()));
        rows
    }

    fn row<'a>(&'a self, service: &str) -> Option<Row<'a>> {
        self.services
            .iter()
            .find(|summary| summary.service == service)
            .map(Row::Listed)
            .or_else(|| match &self.hidden {
                Hidden::These(defined) => defined
                    .iter()
                    .find(|defined| defined.name == service)
                    .map(Row::Hidden),
                Hidden::Unknowable => None,
            })
    }

    fn rights_of(&self, service: &str) -> Rights {
        self.rights
            .get(service)
            .cloned()
            .unwrap_or_else(|| Rights::Unknown("they have not been read yet".into()))
    }

    /// Whether `command` is worth offering on `row`, or why not, in words.
    fn offer(&self, command: Command, row: &Row) -> Result<(), String> {
        if self.refused {
            return Err("The service manager does not let you ask it anything.".into());
        }
        if let Some(doing) = self.working.get(row.service()) {
            return Err(format!("{doing} already."));
        }
        if !self.rights_of(row.service()).allow(command) {
            return Err(format!(
                "You are not allowed to {} it.",
                words::verb(command)
            ));
        }
        // A service whose state is hidden may still be the person's to
        // control: what it would come to is peinit's to say.
        let Some(state) = row.state() else {
            return Ok(());
        };
        let state_words = words::state(state).to_lowercase();
        match admission(command, state) {
            Admission::Acts => Ok(()),
            Admission::Already => Err(format!("It is {state_words} already.")),
            Admission::Nothing => Err(format!(
                "It is {state_words}: there is nothing to {}.",
                words::verb(command)
            )),
            Admission::Refused => Err(format!(
                "It cannot be {} while it is {state_words}.",
                words::done(command)
            )),
        }
    }

    /// Picks `service` and asks what it is doing in full.
    fn pick(&mut self, service: String) {
        if self.picked.as_ref() == Some(&service) {
            return;
        }
        self.detail = None;
        self.asking = None;
        self.picked = Some(service.clone());
        // Its state, if it is the person's to ask for, and whether its
        // definition is theirs to change, which is the registry's to say.
        let listed = self.picked().is_some();
        let Some(window) = self.window.upgrade() else {
            return;
        };
        std::thread::spawn(move || {
            let seen = system::look(Some(&service));
            let status = listed.then(|| system::status(&service));
            window.update(|manager, _| {
                manager.seen(seen);
                if let Some(status) = status {
                    manager.detailed(&service, status);
                }
            });
        });
    }

    /// Asks for `command` on `service`, if it is on offer.
    fn command(&mut self, command: Command, service: &str) {
        let Some(row) = self.row(service) else { return };
        let title = row.title().to_string();
        if let Err(why) = self.offer(command, &row) {
            self.said = Some(Said {
                text: format!("{title} was not {}: {why}", words::done(command)),
                bad: true,
            });
            return;
        }
        let doing = format!("{} {title}", words::doing(command));
        self.working.insert(service.to_string(), doing);
        self.said = None;
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let service = service.to_string();
        std::thread::spawn(move || {
            let going = |operation: &peinit::client::Operation| {
                let now = format!(
                    "{} {title} ({})",
                    words::doing(command),
                    if operation.state == peinit::client::OperationState::Pending {
                        "waiting its turn"
                    } else {
                        "under way"
                    }
                );
                window.update(|manager, _| {
                    manager.working.insert(service.clone(), now);
                });
            };
            let outcome = system::ask(command, &service, going);
            window.update(|manager, _| manager.asked(command, &service, outcome));
            let seen = system::look(Some(&service));
            let status = system::status(&service);
            window.update(|manager, _| {
                manager.seen(seen);
                manager.detailed(&service, status);
            });
        });
    }

    /// What came of `command` on `service`.
    fn asked(&mut self, command: Command, service: &str, outcome: Outcome) {
        self.working.remove(service);
        let title = self
            .row(service)
            .map_or_else(|| service.to_string(), |row| row.title().to_string());
        let (text, bad) = match outcome {
            // A reload nothing confirmed was sent, and may or may not have
            // been done: peinit says which as what it came to.
            Outcome::Done(Some(result))
                if command == Command::Reload && result.contains("advisory") =>
            {
                (
                    format!("{title} was asked to reload, and did not say whether it had."),
                    false,
                )
            }
            Outcome::Done(_) => (format!("{title} was {}.", words::done(command)), false),
            Outcome::Answered(accepted) => (
                format!(
                    "{title} is {} now.",
                    words::state(accepted.state).to_lowercase()
                ),
                false,
            ),
            Outcome::Failed(why) => (
                format!("{title} could not be {}: {why}", words::done(command)),
                true,
            ),
            Outcome::Refused { code, message } => (
                match code.as_str() {
                    "ACCESS_DENIED" => {
                        format!("You are not allowed to {} {title}.", words::verb(command))
                    }
                    "INVALID_STATE" => {
                        format!("{title} cannot be {} as it is now.", words::done(command))
                    }
                    "UNKNOWN_SERVICE" => format!("{title} is no longer defined."),
                    _ => format!("{title} could not be {}: {message}", words::done(command)),
                },
                true,
            ),
            Outcome::StillGoing => (
                format!(
                    "{title} is still being {}. The service manager carries on with it; this window has stopped waiting.",
                    words::done(command)
                ),
                false,
            ),
        };
        self.said = Some(Said { text, bad });
    }

    /// Looks again now, rather than when the next look is due.
    fn refresh(&mut self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let picked = self.picked();
        let chosen = self.chosen();
        std::thread::spawn(move || {
            let seen = system::look(chosen.as_deref());
            let status = picked.map(|picked| {
                let status = system::status(&picked);
                (picked, status)
            });
            window.update(|manager, _| {
                manager.seen(seen);
                if let Some((picked, status)) = status {
                    manager.detailed(&picked, status);
                }
            });
        });
    }

    /// Opens `service`'s definition, or with none a new one, in a window of
    /// its own: this program again, which finds the desktop as this one did.
    /// Nothing ends it when this window goes, so what is being changed there
    /// is not lost with it. The desktop starts programs only for the shell.
    fn definition(&mut self, service: Option<&str>) {
        // One window to a definition: two would each save over the other's
        // changes, or be refused for them. A window cannot be brought
        // forward from here, so it is said instead.
        if let Some(service) = service.filter(|service| self.defining(service)) {
            let title = self
                .row(service)
                .map_or(service.to_string(), |row| row.title().to_string());
            self.said = Some(Said {
                text: format!("The definition of {title} is open already, in a window of its own."),
                bad: false,
            });
            return;
        }
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let program =
            std::env::current_exe().unwrap_or_else(|_| "/usr/bin/gxwi-services-manager".into());
        let arguments = match service {
            Some(service) => vec!["--definition".to_string(), service.to_string()],
            None => vec!["--new".to_string()],
        };
        let open = service.map(str::to_ascii_lowercase);
        match std::process::Command::new(program).args(arguments).spawn() {
            // Waited for, so that it does not linger once it has gone, and
            // then it may be opened again.
            Ok(mut child) => {
                if let Some(open) = &open {
                    self.defining.insert(open.clone());
                }
                let window = Arc::downgrade(&window);
                std::thread::spawn(move || {
                    let _ = child.wait();
                    if let (Some(open), Some(window)) = (open, window.upgrade()) {
                        window.update(|manager, _| {
                            manager.defining.remove(&open);
                        });
                    }
                });
            }
            Err(e) => {
                self.said = Some(Said {
                    text: format!("The definition could not be opened: {e}."),
                    bad: true,
                })
            }
        }
    }

    /// Opens what `service` has logged, and whatever runs under it, in
    /// Event Viewer: a program of its own, whose window outlasts this one.
    fn logs(&mut self, service: &str) {
        match std::process::Command::new(EVENT_VIEWER)
            .args(["--logs", service])
            .spawn()
        {
            // Waited for, so that it does not linger once it has gone.
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.said = Some(Said {
                    text: "Event Viewer is not installed.".into(),
                    bad: true,
                })
            }
            Err(e) => {
                self.said = Some(Said {
                    text: format!("Event Viewer could not be started: {e}."),
                    bad: true,
                })
            }
        }
    }

    /// The button that opens `service`'s definition: to change, or to read.
    fn definition_button(&self, service: &str) -> String {
        let label = match &self.changeable {
            Some((chosen, Ok(()))) if chosen == service => "Edit definition…",
            _ => "Definition…",
        };
        let open = if self.defining(service) {
            " disabled title=\"Its definition is open already, in a window of its own.\""
        } else {
            ""
        };
        format!(
            "<div class=\"opens\"><button type=\"button\" class=\"open\" fx-click=\"definition\" fx-value-service=\"{service}\"{open}>{label}</button>\
             <button type=\"button\" class=\"open\" fx-click=\"logs\" fx-value-service=\"{service}\">Logs…</button></div>",
            service = escape(service),
        )
    }

    /// Opens the editor on `which` permissions of `service`.
    fn permissions(&mut self, which: Which, service: &str) {
        let (title, open) = match which {
            Which::Every => ("every service".to_string(), (String::new(), which)),
            _ => match self.row(service) {
                Some(row) => (row.title().to_string(), (service.to_string(), which)),
                None => return,
            },
        };
        if self.editing.contains(&open) {
            self.said = Some(Said {
                text: format!(
                    "The permissions saying {} are open already for {title}.",
                    which.says()
                ),
                bad: false,
            });
            return;
        }
        let (request, mut apply) = match permissions::open(which, service, &title) {
            Ok(opened) => opened,
            Err(why) => {
                self.said = Some(Said {
                    text: format!("The permissions of {title} could not be opened: {why}."),
                    bad: true,
                });
                return;
            }
        };
        let looking = self.window.clone();
        let chosen = self.chosen();
        let applied = move |sd: &[u8], parts: &[Part]| {
            apply(sd, parts)?;
            // What the person may do may have changed with it.
            if let Some(window) = looking.upgrade() {
                let seen = system::look(chosen.as_deref());
                window.update(|manager, _| manager.seen(seen));
            }
            Ok(())
        };
        let window = self.window.clone();
        let closed = open.clone();
        let done = move || {
            if let Some(window) = window.upgrade() {
                window.update(|manager, _| {
                    manager.editing.remove(&closed);
                });
            }
        };
        match gxwi_sd_editor::edit(&request, applied, done) {
            Ok(()) => {
                self.editing.insert(open);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.said = Some(Said {
                    text: "The permissions editor is not installed.".into(),
                    bad: true,
                })
            }
            Err(e) => {
                self.said = Some(Said {
                    text: format!("The permissions editor could not be started: {e}."),
                    bad: true,
                })
            }
        }
    }

    /// The buttons that open `service`'s permissions, and where who may
    /// control it comes from; and the default every service without its
    /// own takes, with the buttons that open and forget that.
    fn permission_buttons(&self, service: &str) -> String {
        let (from, control) = match (self.from.get(service), self.rights_of(service)) {
            (Some(from), _) => (permissions::from_words(*from).to_string(), String::new()),
            (None, Rights::Unknown(why)) => (
                format!("Who may control it could not be read: {why}."),
                " disabled".to_string(),
            ),
            (None, Rights::Granted(_)) => (String::new(), String::new()),
        };
        let service_html = escape(service);
        // A button that sets something back to the default, asked about
        // first, and offered where it may be done.
        let forget = |forget: Forget,
                      label: &str,
                      changeable: Result<(), String>,
                      question: String,
                      keep: &str| {
            if self.asking.as_ref() == Some(&forget) {
                return format!(
                    "<div class=\"asking\" role=\"group\" aria-label=\"{label}\"><p>{question}</p>\
                     <button type=\"button\" fx-click=\"forget-yes\" fx-value-service=\"{service_html}\" fx-autofocus>{label}</button>\
                     <button type=\"button\" fx-click=\"forget-no\" fx-value-service=\"{service_html}\">{keep}</button></div>",
                    question = escape(&question),
                );
            }
            let which = if forget == Forget::Every {
                "every"
            } else {
                "service"
            };
            match changeable {
                Ok(()) => format!(
                    "<button type=\"button\" fx-click=\"forget\" fx-value-which=\"{which}\" fx-value-service=\"{service_html}\">{label}</button>"
                ),
                Err(why) => format!(
                    "<button type=\"button\" disabled title=\"{}\">{label}</button>",
                    escape(&words::sentence(&why))
                ),
            }
        };
        let own = if self.from.get(service) == Some(&From::Own) {
            let changeable = match &self.changeable {
                Some((chosen, changeable)) if chosen == service => changeable.clone(),
                _ => Err("whether you may has not been found out yet".into()),
            };
            let takes = match self.every {
                Ok(From::AllServices) => "the default set for every service",
                _ => "the service manager's built-in default",
            };
            let title = self
                .row(service)
                .map_or_else(|| service.to_string(), |row| row.title().to_string());
            forget(
                Forget::Service(service.to_string()),
                "Use the default",
                changeable,
                format!(
                    "{title} will take {takes}. Who may control it as set for it now will be lost."
                ),
                "Keep its own",
            )
        } else {
            String::new()
        };
        let (every, every_set) = match &self.every {
            Ok(from) => (
                permissions::every_words(*from).to_string(),
                *from == From::AllServices,
            ),
            Err(why) => (
                format!(
                    "What a service without permissions of its own takes could not be read: {why}."
                ),
                false,
            ),
        };
        let builtin = if every_set {
            let question = "Every service without permissions of its own will take the service manager's built-in default, and the default set for every service will be lost.".to_string();
            forget(
                Forget::Every,
                "Use the built-in default",
                self.every_changeable.clone(),
                question,
                "Keep it",
            )
        } else {
            String::new()
        };
        let every_off = if self.every.is_err() { " disabled" } else { "" };
        format!(
            "<section class=\"permissions\" aria-label=\"Permissions\"><p class=\"note\">{from}</p>\
             <div class=\"together\"><button type=\"button\" fx-click=\"permissions\" fx-value-which=\"control\" fx-value-service=\"{service_html}\"{control}>Who may control it…</button>{own}</div>\
             <button type=\"button\" fx-click=\"permissions\" fx-value-which=\"definition\" fx-value-service=\"{service_html}\">Who may change its definition…</button>\
             </section>\
             <section class=\"permissions every\" aria-labelledby=\"every\"><h3 id=\"every\">Every service</h3><p class=\"note\">{every}</p>\
             <div class=\"together\"><button type=\"button\" fx-click=\"permissions\" fx-value-which=\"every\" fx-value-service=\"{service_html}\"{every_off}>Default permissions…</button>{builtin}</div>\
             </section>",
            from = escape(&from),
            every = escape(&every),
        )
    }

    /// Takes away what `forget` names, so that the default applies, and
    /// looks again.
    fn forget(&mut self, forget: Forget) {
        self.asking = None;
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let chosen = self.chosen();
        std::thread::spawn(move || {
            let (service, said) = match &forget {
                Forget::Service(service) => (Some(service.as_str()), "now takes the default"),
                Forget::Every => (None, ""),
            };
            let done = system::forget(service);
            let seen = system::look(chosen.as_deref());
            window.update(|manager, _| {
                let title = service.map(|service| manager.row(service).map_or_else(|| service.to_string(), |row| row.title().to_string()));
                manager.said = Some(match (done, title) {
                    (Ok(()), Some(title)) => Said { text: format!("{title} {said}."), bad: false },
                    (Ok(()), None) => Said { text: "Every service without permissions of its own now takes the service manager's built-in default.".into(), bad: false },
                    (Err(why), Some(title)) => Said { text: format!("{title} could not be set back to the default: {why}."), bad: true },
                    (Err(why), None) => Said { text: format!("The default for every service could not be taken away: {why}."), bad: true },
                });
                manager.seen(seen);
            });
        });
    }

    /// The buttons for `row`'s commands, each pressable or saying why not.
    fn buttons(&self, row: Option<&Row>) -> String {
        COMMANDS
            .iter()
            .map(|&command| {
                let name = words::command(command);
                match row.map(|row| (row, self.offer(command, row))) {
                    Some((row, Ok(()))) => format!(
                        "<button type=\"button\" fx-click=\"command\" fx-value-command=\"{}\" fx-value-service=\"{}\">{name}</button>",
                        words::verb(command),
                        escape(row.service()),
                    ),
                    Some((_, Err(why))) => format!("<button type=\"button\" disabled title=\"{}\">{name}</button>", escape(&why)),
                    None => format!("<button type=\"button\" disabled title=\"Pick a service first.\">{name}</button>"),
                }
            })
            .collect()
    }

    /// The menu of `row`'s commands.
    fn menu(&self, index: usize, row: &Row) -> String {
        let items: String = COMMANDS
            .iter()
            .map(|&command| {
                let disabled = if self.offer(command, row).is_ok() { "" } else { " disabled" };
                format!("<li><button type=\"button\" fx-click=\"command\" fx-value-command=\"{}\"{disabled}>{}</button></li>", words::verb(command), words::command(command))
            })
            .collect();
        let control = if self.from.contains_key(row.service()) {
            ""
        } else {
            " disabled"
        };
        format!(
            "<menu id=\"menu-{index}\" hidden>{items}<hr>\
             <li><button type=\"button\" fx-click=\"permissions\" fx-value-which=\"control\"{control}>Who may control it…</button></li>\
             <li><button type=\"button\" fx-click=\"permissions\" fx-value-which=\"definition\">Who may change its definition…</button></li><hr>\
             <li><button type=\"button\" fx-click=\"definition\"{defining}>Definition…</button></li>\
             <li><button type=\"button\" fx-click=\"logs\">Logs…</button></li>\
             <li><button type=\"button\" fx-copy=\"service\">Copy name</button></li></menu>",
            defining = if self.defining(row.service()) {
                " disabled"
            } else {
                ""
            },
        )
    }

    /// Whether `service`'s definition is open in a window already.
    fn defining(&self, service: &str) -> bool {
        self.defining.contains(&service.to_ascii_lowercase())
    }

    /// What may be done with `service`, in a sentence.
    fn may(&self, service: &str) -> String {
        match self.rights_of(service) {
            Rights::Granted(granted) => {
                let may: Vec<&str> = [Command::Start, Command::Stop, Command::Reload]
                    .into_iter()
                    .filter(|&command| Rights::Granted(granted).allow(command))
                    .map(words::verb)
                    .collect();
                match may.as_slice() {
                    [] => "You may not start, stop or reload it.".into(),
                    [one] => format!("You may {one} it, and nothing else."),
                    [first, second] => format!("You may {first} and {second} it."),
                    _ => "You may start, stop and reload it.".into(),
                }
            }
            Rights::Unknown(why) => format!(
                "Whether you may control it is not known: {why}. Its commands are offered, and the service manager decides."
            ),
        }
    }

    /// Its calendar timers, each under its schedule: when it runs next and
    /// when it last ran, or why it never runs. As peinit has them armed, so
    /// a timer's random delay is in its next run already.
    fn timers(&self, timers: &[Timer]) -> String {
        if timers.is_empty() {
            return String::new();
        }
        let (now, zone) = self.clock.now();
        let when = |at: &str| words::capital(&words::when(at, now, &zone));
        let each: String = timers
            .iter()
            .map(|timer| {
                let said = match (&timer.not_armed, &timer.fires_at) {
                    (Some(why), _) => format!("<p class=\"note bad\">It never runs: {}.</p>", escape(why)),
                    (None, Some(fires)) => {
                        let next = match words::until(fires, now) {
                            Some(until) => format!("{}, in {until}", when(fires)),
                            None => "Now".to_string(),
                        };
                        let last = timer.last_fired_at.as_deref().map_or_else(|| "Not since the machine started".to_string(), when);
                        let delayed = match &timer.scheduled_at {
                            Some(scheduled) if scheduled != fires => format!(
                                "<p class=\"note\">It is due {}, and put back by a random delay.</p>",
                                escape(&words::when(scheduled, now, &zone))
                            ),
                            _ => String::new(),
                        };
                        format!("<dl><dt>Next run</dt><dd>{}</dd><dt>Last run</dt><dd>{}</dd></dl>{delayed}", escape(&next), escape(&last))
                    }
                    (None, None) => String::new(),
                };
                format!("<div class=\"timer\"><p class=\"schedule\"><code>{}</code></p>{said}</div>", escape(&timer.schedule))
            })
            .collect();
        format!(
            "<section class=\"timers\"><h3>{}</h3>{each}</section>",
            if timers.len() == 1 { "Timer" } else { "Timers" }
        )
    }

    /// The pane about the picked service.
    fn details(&self) -> String {
        let Some(row) = self.picked.as_deref().and_then(|picked| self.row(picked)) else {
            return "<aside class=\"details empty\" aria-label=\"Details\"><p>Pick a service to see what it is doing.</p></aside>".into();
        };
        let row_of = |name: &str, value: &str| format!("<dt>{name}</dt><dd>{}</dd>", escape(value));
        let mut facts = String::new();
        let mut notes = String::new();
        let mut timers = String::new();
        // Its name goes under its title where the title is another.
        let (id, description) = (row.display_name().map(|_| row.service()), row.description());
        if let Row::Listed(summary) = &row {
            let state = match &summary.cause {
                Some(cause) => format!("{}, {}", words::state(summary.state), words::cause(cause)),
                None => words::state(summary.state).to_string(),
            };
            facts += &row_of("State", &state);
            if let Some(health) = summary.health {
                facts += &row_of("Health", words::health(health));
            }
        }
        match self
            .detail
            .as_ref()
            .filter(|detail| detail.service == row.service())
            .map(|detail| &detail.status)
        {
            Some(Ok(status)) => {
                if let Some(job) = &status.job {
                    if let Some(pid) = job.pid {
                        facts += &row_of("Process", &pid.to_string());
                    }
                    facts += &row_of("Runs as", &words::principal(&job.identity));
                }
                if let Some(uptime) = status.uptime_seconds {
                    facts += &row_of("Running for", &words::duration(uptime));
                }
                if let Some(said) = &status.status_text {
                    facts += &row_of("It says", said);
                }
                if let Some(operation) = &status.operation {
                    facts += &row_of(
                        "Under way",
                        &format!("a {} {}", operation.kind, words::source(&operation.source)),
                    );
                }
                if status.definition_removed {
                    notes += "<p class=\"note\">Its definition has been removed. It carries on until it stops, and is gone then.</p>";
                }
                for warning in &status.warnings {
                    notes += &format!(
                        "<p class=\"note bad\">An earlier run left processes behind, in {}.</p>",
                        escape(&warning.path)
                    );
                }
                timers = self.timers(&status.timers);
            }
            Some(Err(why)) => notes += &format!("<p class=\"note\">{}</p>", escape(why)),
            None if matches!(row, Row::Listed(_)) => {
                notes += "<p class=\"note\">Asking what it is doing…</p>"
            }
            None => {}
        }
        if matches!(row, Row::Hidden(_)) {
            notes += "<p class=\"note\">Its state is not yours to see. It is listed because you may read which services are defined.</p>";
        }
        let may = if self.refused {
            "The service manager does not let you ask it anything.".to_string()
        } else {
            self.may(row.service())
        };
        format!(
            "<aside class=\"details\" aria-label=\"Details\">\
             <h2>{title}</h2>{id}{description}\
             <dl>{facts}</dl>{notes}{timers}{definition}\
             <p class=\"may\">{may}</p>{permissions}\
             </aside>",
            definition = self.definition_button(row.service()),
            title = escape(row.title()),
            id = id
                .map(|id| format!("<p class=\"id\">{}</p>", escape(id)))
                .unwrap_or_default(),
            description = description
                .map(|description| format!("<p class=\"description\">{}</p>", escape(description)))
                .unwrap_or_default(),
            may = escape(&may),
            permissions = self.permission_buttons(row.service()),
        )
    }

    /// The footer: how many, and what is not listed.
    fn footer(&self, shown: usize) -> String {
        let running = self
            .services
            .iter()
            .filter(|service| matches!(service.state, State::Active | State::Reloading))
            .count();
        if self.refused {
            let defined = match self.hidden_count() {
                1 => "1 service defined".to_string(),
                count => format!("{count} services defined"),
            };
            let defined = if self.hidden == Hidden::Unknowable {
                String::new()
            } else {
                defined
            };
            return format!(
                "<footer class=\"status\"><span>{defined}</span><span>Their state is not yours to see</span></footer>"
            );
        }
        if !self.looked || (self.trouble.is_some() && self.services.is_empty()) {
            return "<footer class=\"status\"><span></span><span></span></footer>".into();
        }
        let counted = if shown == self.services.len() + self.hidden_count() {
            format!("{} services · {running} running", self.services.len())
        } else {
            format!(
                "{shown} shown of {} · {running} running",
                self.services.len() + self.hidden_count()
            )
        };
        let hidden = match &self.hidden {
            Hidden::These(names) if names.is_empty() => String::new(),
            Hidden::These(names) if names.len() == 1 => "1 more whose state you may not see".into(),
            Hidden::These(names) => format!("{} more whose state you may not see", names.len()),
            Hidden::Unknowable => "Services whose state you may not see are not listed".into(),
        };
        format!("<footer class=\"status\"><span>{counted}</span><span>{hidden}</span></footer>")
    }

    fn hidden_count(&self) -> usize {
        match &self.hidden {
            Hidden::These(names) => names.len(),
            Hidden::Unknowable => 0,
        }
    }
}

impl Live for Manager {
    fn render(&self, facts: &Facts) -> String {
        let rows = self.rows(facts.fields.get("find"));
        let (now, zone) = self.clock.now();
        let listing: String = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let (tone, state) = match (row.state(), self.working.get(row.service())) {
                    (_, Some(doing)) => ("busy", doing.clone()),
                    (Some(state), None) => (words::tone(state), words::state(state).to_string()),
                    (None, None) => ("unknown", "Not yours to see".to_string()),
                };
                let description = row.description().unwrap_or("");
                let next = match row {
                    Row::Listed(Summary { next_timer_at: Some(next), .. }) => words::capital(&words::when(next, now, &zone)),
                    _ => String::new(),
                };
                format!(
                    "<li><button type=\"button\" id=\"service-{index}\" fx-click=\"pick\" fx-menu=\"menu-{index}\" fx-value-service=\"{service}\" aria-selected=\"{picked}\">\
                     <span class=\"dot {tone}\" aria-hidden=\"true\"></span><span class=\"name\">{title}</span>\
                     <span class=\"state\">{state}</span><span class=\"next\">{next}</span><span class=\"description\">{description}</span>\
                     </button></li>",
                    service = escape(row.service()),
                    picked = self.picked.as_deref() == Some(row.service()),
                    title = escape(row.title()),
                    state = escape(&state),
                    next = escape(&next),
                    description = escape(description),
                )
            })
            .collect();
        let menus: String = rows
            .iter()
            .enumerate()
            .map(|(index, row)| self.menu(index, row))
            .collect();
        let picked = self.picked.as_deref().and_then(|picked| self.row(picked));
        let at = picked.as_ref().and_then(|picked| {
            rows.iter()
                .position(|row| row.service() == picked.service())
        });
        let near = |step: isize| {
            let to = match at {
                Some(at) => at
                    .saturating_add_signed(step)
                    .min(rows.len().saturating_sub(1)),
                None => 0,
            };
            rows.get(to)
                .map(|row| escape(row.service()))
                .unwrap_or_default()
        };
        let trouble = self
            .trouble
            .as_ref()
            .map(|why| format!("<p class=\"trouble\">{}</p>", escape(why)))
            .unwrap_or_default();
        let said = self
            .said
            .as_ref()
            .map(|said| {
                format!(
                    "<p class=\"said{}\" role=\"status\">{}</p>",
                    if said.bad { " bad" } else { "" },
                    escape(&said.text)
                )
            })
            .unwrap_or_default();
        let empty = match (self.looked, rows.is_empty(), self.trouble.is_some()) {
            (false, _, false) => "<p class=\"more\">Asking the service manager…</p>",
            (true, true, false) if !facts.fields.get("find").trim().is_empty() => {
                "<p class=\"more\">No service is called that.</p>"
            }
            (true, true, false) => "<p class=\"more\">There are no services you may see.</p>",
            _ => "",
        };
        format!(
            "<div hidden>\
             <button type=\"button\" fx-key=\"ArrowDown\" fx-click=\"pick\" fx-value-service=\"{next}\"></button>\
             <button type=\"button\" fx-key=\"ArrowUp\" fx-click=\"pick\" fx-value-service=\"{previous}\"></button>\
             </div>\
             <div class=\"bar\">\
             <input name=\"find\" autocomplete=\"off\" spellcheck=\"false\" placeholder=\"Find a service\" aria-label=\"Find a service\">\
             <span class=\"commands\">{buttons}</span>\
             <button type=\"button\" class=\"refresh\" fx-click=\"refresh\" fx-key=\"F5\" title=\"Look again (F5)\">Refresh</button>\
             {new}\
             </div>\
             <div class=\"body\">\
             <div class=\"listing\" id=\"listing\" fx-columns=\"10px minmax(0, 1.3fr) 150px 140px minmax(0, 2fr)\">\
             <div class=\"head\"><span></span><span>Name</span><span>State</span><span>Next run</span><span>Description</span></div>\
             {trouble}{said}<ul class=\"entries\">{listing}</ul>{empty}\
             </div>\
             {details}\
             </div>\
             {footer}{menus}",
            next = near(1),
            previous = near(-1),
            new = match &self.creatable {
                Ok(()) =>
                    "<button type=\"button\" class=\"new\" fx-click=\"new\">New service…</button>"
                        .to_string(),
                Err(why) => format!(
                    "<button type=\"button\" class=\"new\" disabled title=\"{}\">New service…</button>",
                    escape(&words::sentence(why))
                ),
            },
            buttons = self.buttons(picked.as_ref()),
            details = self.details(),
            footer = self.footer(rows.len()),
        )
    }

    fn event(&mut self, name: &str, value: &Value, _: &mut Fields) {
        let service = value["service"]
            .as_str()
            .filter(|service| !service.is_empty())
            .map(str::to_string);
        match name {
            "pick" => {
                if let Some(service) = service {
                    self.pick(service);
                }
            }
            "command" => {
                if let (Some(command), Some(service)) =
                    (value["command"].as_str().and_then(words::named), service)
                {
                    self.pick(service.clone());
                    self.command(command, &service);
                }
            }
            "permissions" => {
                if let (Some(which), Some(service)) =
                    (value["which"].as_str().and_then(Which::named), service)
                {
                    self.pick(service.clone());
                    self.permissions(which, &service);
                }
            }
            "forget" => {
                self.asking = match (value["which"].as_str(), service) {
                    (Some("every"), _) => Some(Forget::Every),
                    (Some("service"), Some(service))
                        if self.from.get(&service) == Some(&From::Own) =>
                    {
                        Some(Forget::Service(service))
                    }
                    _ => None,
                };
            }
            "forget-yes" => {
                if let Some(forget) = self.asking.take() {
                    self.forget(forget);
                }
            }
            "forget-no" => self.asking = None,
            "definition" => {
                if let Some(service) = service {
                    self.pick(service.clone());
                    self.definition(Some(&service));
                }
            }
            "logs" => {
                if let Some(service) = service {
                    self.pick(service.clone());
                    self.logs(&service);
                }
            }
            "new" if self.creatable.is_ok() => self.definition(None),
            "refresh" => {
                self.said = None;
                self.refresh();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
