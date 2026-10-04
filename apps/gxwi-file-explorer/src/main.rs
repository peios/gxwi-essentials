//! Gexora — a file explorer: one window showing a directory, with a way into
//! each thing in it.
//!
//! It is an app like any other, drawn by this process for whoever is looking.
//! Where it is looking, which entry is picked and how the listing is sorted
//! are the window's, so two devices showing the one window see the same
//! directory: another directory at the same time is another window.
//!
//! Every entry is drawn with its icon as PGSS Icons finds it, which is what
//! this is first for: a directory full of things is the icon rules tried all
//! at once.
//!
//! OPENING. A directory is gone into and a program is started. Anything
//! else is opened with an application that says it opens files of its type
//! (PGSS Applications): the candidates are the applications in the
//! catalogue whose `opens` the type matches. Opening a file uses the type's
//! default, the application the person chose for it, or else the one chosen
//! for the machine (PGSS Applications §5.7), and the first of the
//! candidates by title when nobody has chosen one that opens it. An
//! entry's menu offers every candidate by name, the default first and the
//! rest by title. The choices are read again whenever a file is opened, so
//! a change to them is used at once; the menus show them as they were when
//! the directory was last read, or a file last opened. Gexora is
//! no part of the shell, so it starts what it opens itself, as a child of
//! its own on the desktop it is on. A file whose name says nothing of its
//! type is opened as text if it reads as text, which is found out when it
//! is opened and not before: a listing reads no file.
//!
//! AN ENTRY'S MENU also renames it, copies its path, and deletes it. There
//! is nowhere for what is deleted to go, so deleting asks first, and a
//! directory goes with everything in it. A new name that is already
//! something's is refused: nothing is ever written over.
//!
//! CUTTING, COPYING AND PASTING. An entry is cut or copied from its menu,
//! and pasted from the menu of the directory, which is the menu of the
//! listing wherever no entry is. What is held is the session's and not the
//! window's (`held`), so that it is pasted into another window. A copy
//! pasted where its name is taken is given a name that says it is a copy;
//! what was cut is not moved onto a name that is taken, and is held until
//! it has been pasted. A directory is copied with everything in it, which
//! may take a while: the window says what it is busy with and carries on
//! being a window.
//!
//! THE DIRECTORY'S MENU also makes a folder or a file, its name typed where
//! it will be listed, and opens the directory with whatever says it opens
//! directories. That is how a terminal is opened here.
//!
//! PERMISSIONS. An entry's menu opens its permissions in gxwi-sd-editor, a
//! dialog of its own that this starts and speaks for (its library): the
//! owner, and who may do what with it. Gexora reads the descriptor and
//! applies what the person applies, through a handle it holds on the entry
//! while the dialog is open, which can do nothing but that. So what is
//! applied is applied to the entry it was opened on, wherever it has been
//! moved since, and never to something that has taken its name. One entry's
//! permissions are open once at a time.

use std::collections::HashSet;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Weak;
use std::time::SystemTime;

use gxwi_sd_editor::{Can, Children, Generic, Object, Part, Request, Right};
use libgxwi::apps::defaults::Defaults;
use libgxwi::apps::{Application, Catalogue};
use peios::access::AccessCheck;
use peios::file::{File, SecInfo};
use peios::security::{AccessMask, SecurityDescriptor};
use libgxwi::icons::{self, Themes};
use libgxwi::{App, Facts, Fields, Live, Surface, Value, escape};

mod held;
mod show;
mod work;

use held::{Held, Holding};

// What this program looks like, to whatever lists it. The icon itself is
// `gexora.svg` at the repo root, installed as the base theme's.
libgxwi::icon!(b"dev.peios.gexora");

/// The most entries drawn at once. A directory with more says so.
const MOST: usize = 2000;

struct Entry {
    name: String,
    directory: bool,
    /// Whether opening it starts it: a regular file this process may run.
    program: bool,
    icon: String,
    /// Its media type, as far as libgxwi can tell from its name.
    media: Option<String>,
    size: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
}

impl Entry {
    /// Its kind: a folder, its media type, or a file. What a type is called
    /// waits for something that says.
    fn kind(&self) -> &str {
        if self.directory { "Folder" } else { self.media.as_deref().unwrap_or("File") }
    }

    /// The media type an application is asked about to open it.
    fn opened_as(&self) -> Option<&str> {
        if self.directory { Some("inode/directory") } else { self.media.as_deref() }
    }
}

/// What is being asked of the person before anything more is done.
#[derive(Clone, PartialEq)]
enum Asking {
    /// A new name for the entry of this name, typed where the name was.
    Rename(String),
    /// Whether the entry of this name is to be deleted.
    Delete(String),
    /// A name for a new folder, or a new file, typed where it will be
    /// listed.
    New { directory: bool },
}

impl Asking {
    /// The entry it is about, if it is about one.
    fn about(&self) -> Option<&str> {
        match self {
            Asking::Rename(name) | Asking::Delete(name) => Some(name),
            Asking::New { .. } => None,
        }
    }
}

/// Somewhere to go from the sidebar.
struct Place {
    name: String,
    path: PathBuf,
    icon: String,
}

/// A volume: a mounted filesystem that holds things, and how full it is.
struct Volume {
    name: String,
    path: PathBuf,
    space: Option<(u64, u64)>,
}

/// Where the choices of which application opens a type come from.
enum Choosing {
    /// The registry, as whoever this runs as: theirs, and the machine's.
    Registry,
    /// These, whatever the registry says.
    #[cfg_attr(not(test), allow(dead_code))]
    Fixed(Defaults),
}

impl Choosing {
    fn read(&self) -> Defaults {
        match self {
            Choosing::Registry => Defaults::read(),
            Choosing::Fixed(defaults) => defaults.clone(),
        }
    }
}

/// What the listing is sorted by. Directories come first whatever it is.
#[derive(Clone, Copy, PartialEq)]
enum By {
    Name,
    Modified,
    Kind,
    Size,
}

impl By {
    fn named(name: &str) -> Option<By> {
        match name {
            "name" => Some(By::Name),
            "modified" => Some(By::Modified),
            "kind" => Some(By::Kind),
            "size" => Some(By::Size),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            By::Name => "name",
            By::Modified => "modified",
            By::Kind => "kind",
            By::Size => "size",
        }
    }
}

struct Explorer {
    themes: Themes,
    at: PathBuf,
    /// The directory as last read. Read again on arrival and on asking, not
    /// on every render: a render happens on every keystroke.
    entries: Vec<Entry>,
    /// How many were not drawn, past `MOST`.
    unshown: usize,
    /// Why the directory could not be read, if it could not.
    trouble: Option<String>,
    picked: Option<usize>,
    by: By,
    ascending: bool,
    /// What the volume the directory is on has free, and has, in bytes.
    space: Option<(u64, u64)>,
    places: Vec<Place>,
    volumes: Vec<Volume>,
    catalogue: Catalogue,
    /// The applications that open anything, by title, as the catalogue had
    /// them when the directory was last read.
    openers: Vec<Application>,
    /// Where the choices of what opens a type are read from.
    choosing: Choosing,
    /// The choices, as they were when last read: when the directory was,
    /// or when a file was last opened.
    defaults: Defaults,
    asking: Option<Asking>,
    /// What has been cut or copied.
    holding: Holding,
    /// What the window is busy with, while it is copying or moving.
    working: Option<String>,
    /// The entries whose permissions are open, by where they were when
    /// they were opened.
    editing: HashSet<PathBuf>,
    /// The window, for work done aside to say when it is done. Without one
    /// the work is done before anything else is.
    window: Weak<Surface<Explorer>>,
}

impl Explorer {
    fn new(at: PathBuf) -> Explorer {
        Explorer::with(at, Catalogue::system(), Choosing::Registry, Holding::session())
    }

    fn with(at: PathBuf, catalogue: Catalogue, choosing: Choosing, holding: Holding) -> Explorer {
        let mut explorer = Explorer {
            themes: Themes::active(),
            catalogue,
            openers: Vec::new(),
            defaults: Defaults::default(),
            choosing,
            asking: None,
            holding,
            working: None,
            editing: HashSet::new(),
            window: Weak::new(),
            at,
            entries: Vec::new(),
            unshown: 0,
            trouble: None,
            picked: None,
            by: By::Name,
            ascending: true,
            space: None,
            places: Vec::new(),
            volumes: Vec::new(),
        };
        explorer.places = places(&explorer.themes);
        explorer.reread();
        explorer
    }

    /// Reads the directory at `self.at`, and says whether it could be opened.
    /// One that cannot be is said, and the last listing stays. One that can
    /// be opened but not read through is shown as far as it was read, and
    /// that is said too.
    fn reread(&mut self) -> bool {
        let listing = match std::fs::read_dir(&self.at) {
            Ok(listing) => listing,
            Err(e) => {
                self.trouble = Some(match e.kind() {
                    std::io::ErrorKind::PermissionDenied => "You are not allowed to see what is in here.".into(),
                    std::io::ErrorKind::NotFound => "There is no such directory.".into(),
                    _ => format!("This directory could not be read: {e}."),
                });
                return false;
            }
        };
        // A listing that fails part way is said as well: what was read is
        // shown, and that it is not everything.
        let mut failed = None;
        let entries: Vec<Entry> = listing
            .filter_map(|entry| entry.map_err(|e| failed = Some(e)).ok())
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let path = entry.path();
                // Through a link, as the thing opened would be.
                let metadata = std::fs::metadata(&path).ok();
                let directory = metadata.as_ref().is_some_and(|m| m.is_dir());
                let program = metadata.as_ref().is_some_and(|m| m.is_file()) && may_run(&path);
                Some(Entry {
                    icon: icons::identify(&path, &self.themes),
                    media: metadata.as_ref().and_then(|m| icons::media_type(&path, m)).filter(|_| !directory),
                    size: metadata.as_ref().map_or(0, |m| m.len()),
                    modified: metadata.as_ref().and_then(|m| m.modified().ok()),
                    created: metadata.as_ref().and_then(|m| m.created().ok()),
                    name,
                    directory,
                    program,
                })
            })
            .collect();
        self.unshown = entries.len().saturating_sub(MOST);
        self.trouble = failed.map(|e| match (e.kind(), entries.is_empty()) {
            (std::io::ErrorKind::PermissionDenied, true) => "You are not allowed to see what is in here.".into(),
            _ => format!("Not everything in here could be read: {e}."),
        });
        self.entries = entries;
        self.sort();
        self.entries.truncate(MOST);
        self.picked = self.picked.filter(|&picked| picked < self.entries.len());
        self.space = space(&self.at);
        self.volumes = volumes();
        self.openers = self.catalogue.read().into_iter().filter(|application| !application.opens.is_empty()).collect();
        self.openers.sort_by_cached_key(|application| (application.title.to_lowercase(), application.id.clone()));
        self.defaults = self.choosing.read();
        // What was being asked about may have gone.
        self.asking = self.asking.take().filter(|asking| asking.about().is_none_or(|name| self.entries.iter().any(|entry| entry.name == name)));
        true
    }

    /// The applications that are candidates to open `entry`, first the one
    /// that opening it uses.
    fn candidates(&self, entry: &Entry) -> Vec<&Application> {
        entry.opened_as().map(|media| self.opening(media)).unwrap_or_default()
    }

    /// The applications that say they open `media`: its default first, if
    /// it has one, and the rest by title.
    fn opening(&self, media: &str) -> Vec<&Application> {
        self.defaults.first(media, self.openers.iter().filter(|application| application.opens(media)).collect())
    }

    /// Which entry is called `name`.
    fn named(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|entry| entry.name == name)
    }

    /// Directories first, then by what was asked, then by name without
    /// regard to case, so a listing reads the same wherever it was made.
    fn sort(&mut self) {
        let (by, ascending) = (self.by, self.ascending);
        self.entries.sort_by_cached_key(|entry| (!entry.directory, entry.name.to_lowercase(), entry.name.clone()));
        if by != By::Name {
            self.entries.sort_by(|a, b| {
                let within = match by {
                    By::Modified => a.modified.cmp(&b.modified),
                    By::Kind => a.kind().cmp(b.kind()),
                    By::Size => a.size.cmp(&b.size),
                    By::Name => std::cmp::Ordering::Equal,
                };
                a.directory.cmp(&b.directory).reverse().then(within)
            });
        }
        if !ascending {
            // The other way round within each group, with directories still
            // first: reversing the whole list would put them last.
            let split = self.entries.iter().position(|entry| !entry.directory).unwrap_or(self.entries.len());
            self.entries[..split].reverse();
            self.entries[split..].reverse();
        }
    }

    /// Goes to `to`, if it is a directory that can be opened.
    fn go(&mut self, to: PathBuf, fields: &mut Fields) {
        let was = std::mem::replace(&mut self.at, to);
        self.picked = None;
        self.asking = None;
        if !self.reread() {
            // Said on the listing that stays, over the address that failed.
            self.at = was;
        }
        fields.set("path", &self.at.to_string_lossy());
    }

    fn open(&mut self, index: usize, fields: &mut Fields) {
        // What opens a type is as chosen now, not when the directory was
        // read: a choice is made somewhere else, and should not wait for a
        // refresh here to be used.
        self.defaults = self.choosing.read();
        let Some(entry) = self.entries.get(index) else { return };
        let path = self.at.join(&entry.name);
        if entry.directory {
            self.go(path, fields);
        } else if entry.program {
            if let Err(e) = start(&path, &[]) {
                self.trouble = Some(format!("{} could not be started: {e}.", entry.name));
            }
        } else if let Some(media) = entry.opened_as() {
            match self.opening(media).first().map(|application| application.id.clone()) {
                Some(id) => self.open_with(index, &id, fields),
                None => self.trouble = Some(format!("Nothing installed says it opens {media} files.")),
            }
        } else {
            // Its name says nothing of what it is. If it reads as text,
            // what opens text opens it.
            let text = work::reads_as_text(&path).then(|| self.opening("text/plain").first().map(|application| application.command(Some(&path)))).flatten();
            match text {
                Some((program, arguments)) => {
                    if let Err(e) = start(&program, &arguments) {
                        self.trouble = Some(format!("{} could not be opened: {e}.", entry.name));
                    }
                }
                None => self.trouble = Some("Nothing installed says it opens files like this.".into()),
            }
        }
    }

    /// Opens the directory being shown with the application `id`, if it
    /// says it opens directories: a terminal here, or another window.
    fn open_here(&mut self, id: &str) {
        let Some(application) = self.catalogue.get(id).filter(|application| application.opens("inode/directory")) else {
            self.trouble = Some("This directory cannot be opened with that.".into());
            return;
        };
        let (program, arguments) = application.command(Some(&self.at));
        if let Err(e) = start(&program, &arguments) {
            self.trouble = Some(format!("{} could not be started: {e}.", application.title));
        }
    }

    /// Opens the entry with the application `id`, as the catalogue has it
    /// now, if it is a candidate for it. A directory opened with Gexora is
    /// another window, like anything else opened with anything.
    fn open_with(&mut self, index: usize, id: &str, _: &mut Fields) {
        let Some(entry) = self.entries.get(index) else { return };
        let Some(application) = self.catalogue.get(id).filter(|application| entry.opened_as().is_some_and(|media| application.opens(media))) else {
            self.trouble = Some(format!("{} cannot be opened with that.", entry.name));
            return;
        };
        let (program, arguments) = application.command(Some(&self.at.join(&entry.name)));
        if let Err(e) = start(&program, &arguments) {
            self.trouble = Some(format!("{} could not be started: {e}.", application.title));
        }
    }

    /// Gives the entry called `was` the name `typed`, if that is a name and
    /// nothing has it.
    fn rename(&mut self, was: &str, typed: &str) {
        let to = typed.trim();
        if to == was {
            return;
        }
        if !work::is_name(to) {
            self.trouble = Some("That is not a name a file can have.".into());
            return;
        }
        let target = self.at.join(to);
        // Asked of the thing itself and not what it may link to: a link
        // that goes nowhere still has the name.
        if work::taken(&target) {
            self.trouble = Some(format!("There is already something called {to} here."));
            return;
        }
        match std::fs::rename(self.at.join(was), &target) {
            Ok(()) => {
                self.reread();
                self.picked = self.named(to);
            }
            Err(e) => self.trouble = Some(format!("{was} could not be renamed: {}.", why(&e))),
        }
    }

    /// Deletes the entry called `name`: a directory with everything in it,
    /// and a link as the link it is.
    fn delete(&mut self, name: &str) {
        let removed = work::delete(&self.at.join(name));
        self.reread();
        if let Err(e) = removed {
            self.trouble = Some(format!("{name} could not be deleted: {}.", why(&e)));
        }
    }

    /// Makes a folder or a file called what was typed, with nothing in it,
    /// if that is a name and nothing has it.
    fn make(&mut self, typed: &str, directory: bool) {
        let name = typed.trim();
        if !work::is_name(name) {
            self.trouble = Some("That is not a name a file can have.".into());
            return;
        }
        let path = self.at.join(name);
        let made = if directory {
            std::fs::create_dir(&path)
        } else {
            std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map(|_| ())
        };
        match made {
            Ok(()) => {
                self.reread();
                self.picked = self.named(name);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => self.trouble = Some(format!("There is already something called {name} here.")),
            Err(e) => self.trouble = Some(format!("{name} could not be made: {}.", why(&e))),
        }
    }

    /// Holds the entry called `name`, to be copied or moved to wherever it
    /// is pasted.
    fn hold(&mut self, name: &str, cut: bool) {
        if let Err(e) = self.holding.set(Some(Held { path: self.at.join(name), cut })) {
            self.trouble = Some(format!("{name} could not be {}: {e}.", if cut { "cut" } else { "copied" }));
        }
    }

    /// Puts what is held in the directory being shown.
    fn paste(&mut self) {
        if let Some(working) = &self.working {
            self.trouble = Some(format!("Still {working}."));
            return;
        }
        let Some(held) = self.holding.get() else {
            self.trouble = Some("Nothing has been cut or copied.".into());
            return;
        };
        let name = held.path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        let Ok(metadata) = std::fs::symlink_metadata(&held.path) else {
            self.trouble = Some(format!("{name} is no longer where it was."));
            let _ = self.holding.set(None);
            return;
        };
        if metadata.is_dir() && self.at.starts_with(&held.path) {
            self.trouble = Some(format!("{name} cannot be put inside itself."));
            return;
        }
        let into = self.at.clone();
        let to = if held.cut {
            // Cut and pasted where it is, it stays where it is.
            if held.path.parent() == Some(into.as_path()) {
                let _ = self.holding.set(None);
                return;
            }
            if work::taken(&into.join(&name)) {
                self.trouble = Some(format!("There is already something called {name} here."));
                return;
            }
            into.join(&name)
        } else {
            work::free(&into, &name)
        };
        let doing = format!("{} {name}", if held.cut { "moving" } else { "copying" });
        let work = move || {
            let done = if held.cut { work::move_to(&held.path, &to) } else { work::copy(&held.path, &to) };
            let said = done.map_err(|e| format!("{name} could not be {}: {}.", if held.cut { "moved" } else { "copied" }, why(&e)));
            (said, to, held.cut)
        };
        match self.window.upgrade() {
            Some(window) => {
                self.working = Some(doing);
                std::thread::spawn(move || {
                    let (said, to, cut) = work();
                    window.update(|explorer, _| explorer.pasted(said, &to, cut));
                });
            }
            None => {
                let (said, to, cut) = work();
                self.pasted(said, &to, cut);
            }
        }
    }

    /// What was pasted is there, or is not and why.
    fn pasted(&mut self, said: Result<(), String>, to: &Path, cut: bool) {
        self.working = None;
        // What was cut is pasted once. What failed part way may be in two
        // places, and is no longer held either: asking again is not the
        // way to put that right.
        if cut {
            let _ = self.holding.set(None);
        }
        self.reread();
        if to.parent() == Some(self.at.as_path()) {
            self.picked = to.file_name().and_then(|name| self.named(&name.to_string_lossy())).or(self.picked);
        }
        if let Err(why) = said {
            self.trouble = Some(why);
        }
    }

    /// Opens the editor on the permissions of the entry at `index`.
    fn permissions(&mut self, index: usize) {
        let Some(entry) = self.entries.get(index) else { return };
        let path = self.at.join(&entry.name);
        if self.editing.contains(&path) {
            self.trouble = Some(format!("The permissions of {} are open already.", entry.name));
            return;
        }
        // A handle for its descriptor and nothing else, through a link as
        // the thing opened would be.
        let handle = match std::fs::OpenOptions::new().read(true).custom_flags(libc::O_PATH).open(&path) {
            Ok(file) => OwnedFd::from(file),
            Err(e) => {
                self.trouble = Some(format!("{} could not be opened: {}.", entry.name, why(&e)));
                return;
            }
        };
        let sd = match peios::file::fd_get_sd(handle.as_fd(), SecInfo::OWNER | SecInfo::GROUP | SecInfo::DACL) {
            Ok(sd) => sd,
            Err(e) => {
                self.trouble = Some(format!("The permissions of {} could not be read: {}.", entry.name, why(&io(e))));
                return;
            }
        };
        let request = Request {
            object: Object { name: entry.name.clone(), kind: if entry.directory { "Folder" } else { "File" }.into(), container: entry.directory, children: Children::All },
            sd: sd.as_bytes().to_vec(),
            rights: file_rights(),
            generic: file_generic(),
            can: can_change(&sd),
        };
        let window = self.window.clone();
        let opened = path.clone();
        let done = move || {
            if let Some(window) = window.upgrade() {
                window.update(|explorer, _| {
                    explorer.editing.remove(&opened);
                });
            }
        };
        match gxwi_sd_editor::edit(&request, move |sd, parts| apply(&handle, sd, parts), done) {
            Ok(()) => {
                self.editing.insert(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.trouble = Some("The permissions editor is not installed.".into()),
            Err(e) => self.trouble = Some(format!("The permissions editor could not be started: {e}.")),
        }
    }

    /// The pane of places to go and volumes to look at.
    fn sidebar(&self) -> String {
        let current = |path: &Path| if path == self.at { " aria-current=\"true\"" } else { "" };
        let places: String = self
            .places
            .iter()
            .map(|place| {
                format!(
                    "<li><button type=\"button\" fx-click=\"visit\" fx-value-path=\"{path}\"{current}><img src=\"{icon}\" alt=\"\">{name}</button></li>",
                    path = escape(&place.path.to_string_lossy()),
                    current = current(&place.path),
                    icon = escape(&icons::href(&place.icon)),
                    name = escape(&place.name),
                )
            })
            .collect();
        let volumes: String = self
            .volumes
            .iter()
            .map(|volume| {
                let (used, note) = match volume.space {
                    Some((free, all)) if all > 0 => (100 - (free * 100 / all).min(100), format!("{} free of {}", show::size(free), show::size(all))),
                    _ => (0, String::new()),
                };
                format!(
                    "<li><button type=\"button\" fx-click=\"visit\" fx-value-path=\"{path}\"{current}>{name}\
                     <progress max=\"100\" value=\"{used}\" aria-hidden=\"true\"></progress><small>{note}</small></button></li>",
                    path = escape(&volume.path.to_string_lossy()),
                    current = current(&volume.path),
                    name = escape(&volume.name),
                )
            })
            .collect();
        format!(
            "<nav class=\"sidebar\" aria-label=\"Places\">\
             <h3>Places</h3><ul>{places}</ul>\
             <h3>Volumes</h3><ul class=\"volumes\">{volumes}</ul>\
             </nav>"
        )
    }

    /// The pane about the picked entry, or nothing.
    fn details(&self) -> String {
        let Some(entry) = self.picked.and_then(|picked| self.entries.get(picked)) else {
            return String::new();
        };
        let row = |name: &str, value: String| format!("<dt>{name}</dt><dd>{}</dd>", escape(&value));
        let size = if entry.directory { "—".into() } else { format!("{} ({} bytes)", show::size(entry.size), show::grouped(entry.size)) };
        let zone = show::here();
        format!(
            "<aside class=\"details\" aria-label=\"Details\">\
             <img src=\"{icon}\" alt=\"\"><h2>{name}</h2>\
             <dl>{kind}{size}{modified}{created}</dl>\
             </aside>",
            icon = escape(&icons::href(&entry.icon)),
            name = escape(&entry.name),
            kind = row("Type", entry.kind().into()),
            size = row("Size", size),
            modified = row("Modified", show::when_long(entry.modified, &zone)),
            created = row("Created", show::when_long(entry.created, &zone)),
        )
    }
}

/// Starts `program`, as this process was started: on the same desktop, and
/// left to itself. Reaped when it ends, so that it does not linger.
fn start(program: &Path, arguments: &[String]) -> std::io::Result<()> {
    let mut child = std::process::Command::new(program)
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// The rights a file or folder has, by the names the permissions editor shows
/// them by, from the most to the least.
fn file_rights() -> Vec<Right> {
    let generic = file_generic();
    let right = |name: &str, mask: u32| Right { name: name.into(), mask, general: true };
    vec![
        right("Full control", generic.all),
        right("Modify", generic.read | generic.write | generic.execute | AccessMask::DELETE.bits()),
        right("Read & execute", generic.read | generic.execute),
        right("Read", generic.read),
        right("Write", generic.write),
    ]
}

/// What the generic rights stand for on a file, as the kernel has it.
fn file_generic() -> Generic {
    let mapping = File::generic_mapping();
    let means = |generic: AccessMask| generic.resolve_generic(&mapping).bits();
    Generic {
        read: means(AccessMask::GENERIC_READ),
        write: means(AccessMask::GENERIC_WRITE),
        execute: means(AccessMask::GENERIC_EXECUTE),
        all: means(AccessMask::GENERIC_ALL),
    }
}

/// What the person can change of a file's descriptor `sd`, as the kernel's
/// AccessCheck says for their token: the access list with WRITE_DAC, the
/// owner with WRITE_OWNER, with what owning it and their privileges give
/// them. It cannot be found out by opening the file for them, as a native
/// open must name a data right too (KACS §3.9.2), and the person may have
/// one without the other. Where the check cannot be made, both are offered,
/// and the kernel decides on Apply.
fn can_change(sd: &SecurityDescriptor) -> Can {
    let granted = match AccessCheck::new(sd, AccessMask::MAXIMUM_ALLOWED, File::generic_mapping()).check() {
        Ok(decision) => decision.granted,
        Err(_) => return Can { dacl: true, owner: true, ..Can::default() },
    };
    let dacl = granted.contains(AccessMask::WRITE_DAC);
    let why = (!dacl).then(|| "You may not change who may use this.".to_string());
    Can { dacl, owner: granted.contains(AccessMask::WRITE_OWNER), audit: false, why }
}

/// Applies the parts of `sd` that changed to what `handle` is open on: what
/// the permissions editor sent. Why not, for the person to read.
fn apply(handle: &OwnedFd, sd: &[u8], parts: &[Part]) -> Result<(), String> {
    let sd = SecurityDescriptor::from_validated_bytes(sd.to_vec()).map_err(|e| format!("what came back is not a security descriptor ({e})"))?;
    let info = parts.iter().fold(SecInfo::empty(), |info, part| {
        info | match part {
            Part::Owner => SecInfo::OWNER,
            Part::Group => SecInfo::GROUP,
            Part::Dacl => SecInfo::DACL,
            Part::Sacl => SecInfo::SACL,
        }
    });
    if info.is_empty() {
        return Ok(());
    }
    peios::file::fd_set_sd(handle.as_fd(), info, &sd).map_err(|e| why(&io(e)))
}

/// A KACS error as the system's own, which is what it carries.
fn io(error: peios::Error) -> std::io::Error {
    error.raw_os_error().map_or_else(|| std::io::Error::new(error.kind(), error.to_string()), std::io::Error::from_raw_os_error)
}

/// Why something could not be done, for the person to read.
fn why(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => "you are not allowed to".into(),
        std::io::ErrorKind::NotFound => "it is no longer there".into(),
        _ => error.to_string(),
    }
}

/// Whether this process may run the file: the system's word, asked the way
/// an exec would ask it.
fn may_run(path: &Path) -> bool {
    let Ok(cpath) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else { return false };
    // SAFETY: a NUL-terminated string, live for the call.
    unsafe { libc::access(cpath.as_ptr(), libc::X_OK) == 0 }
}

/// Home, and the directories in it that a person keeps things in, by the
/// names PGSS Icons knows them by, where they exist.
fn places(themes: &Themes) -> Vec<Place> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from).filter(|home| home.is_dir()) else { return Vec::new() };
    let mut places = vec![Place { name: "Home".into(), icon: icons::identify(&home, themes), path: home.clone() }];
    for name in ["Desktop", "Documents", "Downloads", "Pictures", "Music", "Videos"] {
        let path = home.join(name);
        if path.is_dir() {
            places.push(Place { name: name.into(), icon: icons::identify(&path, themes), path });
        }
    }
    places
}

/// The filesystems mounted that hold things: not the kernel's own, and not
/// the views the system makes of its own directories.
fn volumes() -> Vec<Volume> {
    const NOT_A_VOLUME: &[&str] = &[
        "proc", "sysfs", "devtmpfs", "devpts", "tmpfs", "cgroup", "cgroup2", "stratafs", "securityfs", "debugfs", "tracefs",
        "bpf", "mqueue", "pstore", "configfs", "fusectl", "binfmt_misc", "hugetlbfs", "autofs", "rpc_pipefs", "nsfs", "efivarfs",
    ];
    let Ok(mounts) = std::fs::read_to_string("/proc/self/mountinfo") else { return Vec::new() };
    let mut volumes: Vec<Volume> = mounts
        .lines()
        .filter_map(|line| {
            // ... <mount point> ... - <fstype> <source> <options>
            let (before, after) = line.split_once(" - ")?;
            let point = before.split(' ').nth(4)?;
            let fstype = after.split(' ').next()?;
            if NOT_A_VOLUME.contains(&fstype) {
                return None;
            }
            let path = PathBuf::from(unescape(point));
            let name = if path == Path::new("/") { "This machine".to_string() } else { path.file_name()?.to_string_lossy().into_owned() };
            Some(Volume { space: space(&path), name, path })
        })
        .collect();
    volumes.sort_by(|a, b| (a.path != Path::new("/")).cmp(&(b.path != Path::new("/"))).then(a.path.cmp(&b.path)));
    volumes
}

/// A mount point as mountinfo writes it, with its spaces and such back.
fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let code: String = chars.by_ref().take(3).collect();
            match u32::from_str_radix(&code, 8).ok().and_then(char::from_u32) {
                Some(c) => out.push(c),
                None => out.push_str(&code),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// What the volume `path` is on has free, and has, in bytes.
fn space(path: &Path) -> Option<(u64, u64)> {
    let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    // SAFETY: a zeroed statvfs is a valid one to fill, and the string is
    // NUL-terminated and live for the call.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(cpath.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let unit = stat.f_frsize as u64;
    Some((stat.f_bavail as u64 * unit, stat.f_blocks as u64 * unit))
}

impl Live for Explorer {
    fn render(&self, _: &Facts) -> String {
        let now = SystemTime::now();
        let zone = show::here();
        // What has been cut is shown as on its way out.
        let cut = self.holding.get().filter(|held| held.cut).map(|held| held.path);
        // One menu serves every entry the same applications open, which is
        // most of a directory: the entry's own values come with the choice.
        let mut menus: Vec<Vec<&Application>> = Vec::new();
        let entries: String = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let candidates = self.candidates(entry);
                let menu = menus.iter().position(|menu| menu == &candidates).unwrap_or_else(|| {
                    menus.push(candidates);
                    menus.len() - 1
                });
                let about = format!(
                    "fx-value-index=\"{index}\" fx-value-name=\"{name}\" fx-value-path=\"{path}\"",
                    name = escape(&entry.name),
                    path = escape(&self.at.join(&entry.name).to_string_lossy()),
                );
                let class = match (entry.directory, cut.as_deref() == Some(self.at.join(&entry.name).as_path())) {
                    (true, false) => "directory",
                    (true, true) => "directory cut",
                    (false, false) => "file",
                    (false, true) => "file cut",
                };
                let rest = format!(
                    "<span class=\"modified\">{modified}</span><span class=\"kind\">{kind}</span><span class=\"size\">{size}</span>",
                    modified = show::when(entry.modified, now, &zone),
                    kind = entry.kind(),
                    size = if entry.directory { "—".into() } else { show::size(entry.size) },
                );
                let icon = escape(&icons::href(&entry.icon));
                // Being renamed, its name is a field to type the new one in.
                if self.asking == Some(Asking::Rename(entry.name.clone())) {
                    return format!(
                        "<li><form id=\"renaming-{index}\" class=\"{class} renaming\" fx-submit=\"renamed\" {about}>\
                         <img src=\"{icon}\" alt=\"\"><input name=\"name\" fx-autofocus autocomplete=\"off\" spellcheck=\"false\" aria-label=\"New name for {name}\">\
                         {rest}</form></li>",
                        name = escape(&entry.name),
                    );
                }
                format!(
                    "<li><button type=\"button\" id=\"entry-{index}\" class=\"{class}\" fx-click=\"pick\" fx-dblclick=\"open\" \
                     fx-menu=\"menu-{menu}\" {about} aria-selected=\"{picked}\">\
                     <img src=\"{icon}\" alt=\"\"><span class=\"name\">{name}</span>{rest}\
                     </button></li>",
                    picked = self.picked == Some(index),
                    name = escape(&entry.name),
                )
            })
            .collect();
        let menus: String = menus
            .iter()
            .enumerate()
            .map(|(at, candidates)| {
                let with: String = candidates
                    .iter()
                    .map(|application| {
                        format!(
                            "<li><button type=\"button\" fx-click=\"open-with\" fx-value-app=\"{}\">Open with {}</button></li>",
                            escape(&application.id),
                            escape(&application.title)
                        )
                    })
                    .collect();
                format!(
                    "<menu id=\"menu-{at}\" hidden>\
                     <li><button type=\"button\" fx-click=\"open\">Open</button></li>{with}<hr>\
                     <li><button type=\"button\" fx-click=\"cut\">Cut</button></li>\
                     <li><button type=\"button\" fx-click=\"copy\">Copy</button></li>\
                     <li><button type=\"button\" fx-click=\"rename\">Rename</button></li>\
                     <li><button type=\"button\" fx-copy=\"path\">Copy path</button></li>\
                     <li><button type=\"button\" fx-click=\"permissions\">Permissions…</button></li><hr>\
                     <li><button type=\"button\" fx-click=\"delete\">Delete</button></li>\
                     </menu>"
                )
            })
            .collect();
        // The directory's own menu, which is the listing's wherever no
        // entry is.
        let here: String = self
            .opening("inode/directory")
            .iter()
            .map(|application| {
                format!(
                    "<li><button type=\"button\" fx-click=\"open-here\" fx-value-app=\"{}\">Open with {}</button></li>",
                    escape(&application.id),
                    escape(&application.title)
                )
            })
            .collect();
        let here = format!(
            "<menu id=\"menu-here\" hidden>\
             <li><button type=\"button\" fx-click=\"new-folder\">New folder</button></li>\
             <li><button type=\"button\" fx-click=\"new-file\">New file</button></li>\
             <li><button type=\"button\" fx-click=\"paste\">Paste</button></li><hr>{here}\
             </menu>"
        );
        // Being made, a folder or a file is a field to type its name in,
        // ahead of everything that is there.
        let making = match self.asking {
            Some(Asking::New { directory }) => format!(
                "<li><form id=\"making\" class=\"{class} renaming\" fx-submit=\"made\">\
                 <img src=\"{icon}\" alt=\"\"><input name=\"name\" fx-autofocus autocomplete=\"off\" spellcheck=\"false\" aria-label=\"Name for the new {what}\">\
                 <span class=\"modified\"></span><span class=\"kind\">{kind}</span><span class=\"size\"></span></form></li>",
                class = if directory { "directory" } else { "file" },
                icon = escape(&icons::href(if directory { "inode-directory" } else { icons::GENERIC })),
                what = if directory { "folder" } else { "file" },
                kind = if directory { "Folder" } else { "File" },
            ),
            _ => String::new(),
        };
        // What is asked before something is deleted. The keyboard starts on
        // the way out of it.
        let asking = match &self.asking {
            Some(Asking::Delete(name)) => {
                let directory = self.named(name).is_some_and(|index| self.entries[index].directory);
                format!(
                    "<div id=\"asking\" class=\"asking\" role=\"alertdialog\" aria-label=\"Delete\">\
                     <p>Delete {name}{all}? There is nowhere to get it back from.</p>\
                     <button type=\"button\" class=\"danger\" fx-click=\"deleted\" fx-value-name=\"{name}\">Delete</button>\
                     <button type=\"button\" fx-click=\"keep\" fx-key=\"Escape\" fx-autofocus>Keep</button>\
                     </div>",
                    name = escape(name),
                    all = if directory { " and everything in it" } else { "" },
                )
            }
            _ => String::new(),
        };
        // While something is being asked, the keys are for answering it.
        let keys = match &self.asking {
            Some(Asking::Rename(_) | Asking::New { .. }) => "<button type=\"button\" fx-key=\"Escape\" fx-click=\"keep\"></button>".to_string(),
            Some(Asking::Delete(_)) => String::new(),
            None => format!(
                "<button type=\"button\" fx-key=\"Enter\" fx-click=\"open\" fx-value-index=\"{picked}\"></button>\
                 <button type=\"button\" fx-key=\"ArrowDown\" fx-click=\"next\"></button>\
                 <button type=\"button\" fx-key=\"ArrowUp\" fx-click=\"previous\"></button>\
                 <button type=\"button\" fx-key=\"F2\" fx-click=\"rename\" fx-value-index=\"{picked}\"></button>\
                 <button type=\"button\" fx-key=\"Delete\" fx-click=\"delete\" fx-value-index=\"{picked}\"></button>\
                 <button type=\"button\" fx-key=\"Ctrl+X\" fx-click=\"cut\" fx-value-index=\"{picked}\"></button>\
                 <button type=\"button\" fx-key=\"Ctrl+C\" fx-click=\"copy\" fx-value-index=\"{picked}\"></button>\
                 <button type=\"button\" fx-key=\"Ctrl+V\" fx-click=\"paste\"></button>",
                picked = self.picked.map(|picked| picked.to_string()).unwrap_or_default(),
            ),
        };
        let heading = |by: By, label: &str| {
            let sorted = if self.by == by { if self.ascending { " aria-sort=\"ascending\"" } else { " aria-sort=\"descending\"" } } else { "" };
            format!(
                "<button type=\"button\" class=\"{}\" fx-click=\"sort\" fx-value-by=\"{}\"{sorted}>{label}</button>",
                by.name(),
                by.name()
            )
        };
        let trouble = self.trouble.as_ref().map(|why| format!("<p class=\"trouble\">{}</p>", escape(why))).unwrap_or_default();
        let more = if self.unshown > 0 { format!("<p class=\"more\">and {} more, not shown</p>", self.unshown) } else { String::new() };
        let empty = if self.entries.is_empty() && self.trouble.is_none() && making.is_empty() { "<p class=\"more\">Nothing in here.</p>" } else { "" };
        let counted = match (&self.working, self.picked.and_then(|picked| self.entries.get(picked))) {
            (Some(working), _) => format!("<span class=\"working\">Busy {}</span>", escape(working)),
            (None, Some(entry)) if !entry.directory => format!("{} items · 1 selected · {}", self.entries.len(), show::size(entry.size)),
            (None, Some(_)) => format!("{} items · 1 selected", self.entries.len()),
            (None, None) => format!("{} items", self.entries.len()),
        };
        let space = self.space.map(|(free, all)| format!("{} free of {}", show::size(free), show::size(all))).unwrap_or_default();
        // The first element is never seen. It is what the chords are hung on:
        // Enter opens what is picked, unless it is in the address bar, where
        // it is the form's.
        format!(
            "<div hidden>\
             {keys}\
             <button type=\"button\" fx-key=\"F5\" fx-click=\"reread\"></button>\
             </div>\
             <form class=\"bar\" fx-submit=\"go\">\
             <button type=\"button\" fx-click=\"up\" fx-key=\"Alt+ArrowUp Backspace\" aria-label=\"Up\" title=\"Up (Alt+↑)\">↑</button>\
             <input name=\"path\" autocomplete=\"off\" spellcheck=\"false\" aria-label=\"Path\">\
             </form>\
             <div class=\"body\">\
             {sidebar}\
             <div class=\"listing\" id=\"listing\" fx-menu=\"menu-here\" fx-columns=\"16px minmax(0, 1fr) 140px 130px 80px\">\
             <div class=\"head\"><span></span>{name}{modified}{kind}{size}</div>\
             {trouble}{asking}<ul class=\"entries\">{making}{entries}</ul>{empty}{more}\
             </div>\
             {details}\
             </div>\
             <footer class=\"status\"><span>{counted}</span><span>{space}</span></footer>{menus}{here}",
            name = heading(By::Name, "Name"),
            modified = heading(By::Modified, "Modified"),
            kind = heading(By::Kind, "Type"),
            size = heading(By::Size, "Size"),
            sidebar = self.sidebar(),
            details = self.details(),
        )
    }

    fn event(&mut self, name: &str, value: &Value, fields: &mut Fields) {
        let index = value["index"].as_str().and_then(|index| index.parse().ok()).filter(|&index: &usize| index < self.entries.len());
        let last = self.entries.len().checked_sub(1);
        // What is said of an entry by name is about the entry of that name,
        // wherever the listing has it by now, and about nothing if it has
        // gone: what renames or deletes does not go by a place in a list.
        let index = match value["name"].as_str() {
            Some(name) => self.named(name),
            None => index,
        };
        // Whatever was wrong was wrong with the last thing asked.
        if !matches!(name, "pick" | "next" | "previous" | "sort") {
            self.trouble = None;
        }
        match name {
            "pick" => self.picked = index.or(self.picked),
            "open" => {
                if let Some(index) = index {
                    self.picked = Some(index);
                    self.open(index, fields);
                }
            }
            "open-with" => {
                if let (Some(index), Some(id)) = (index, value["app"].as_str()) {
                    self.picked = Some(index);
                    self.open_with(index, id, fields);
                }
            }
            "rename" => {
                if let Some(index) = index {
                    self.picked = Some(index);
                    fields.set("name", &self.entries[index].name);
                    self.asking = Some(Asking::Rename(self.entries[index].name.clone()));
                }
            }
            "renamed" => {
                if let Some(Asking::Rename(was)) = self.asking.take() {
                    let typed = fields.get("name").to_string();
                    self.rename(&was, &typed);
                }
            }
            "delete" => {
                if let Some(index) = index {
                    self.picked = Some(index);
                    self.asking = Some(Asking::Delete(self.entries[index].name.clone()));
                }
            }
            // Only what was asked about is deleted, and only once.
            "deleted" => {
                if let Some(Asking::Delete(name)) = self.asking.take() {
                    if value["name"].as_str() == Some(name.as_str()) {
                        self.delete(&name);
                    }
                }
            }
            "keep" => self.asking = None,
            "new-folder" | "new-file" => {
                let directory = name == "new-folder";
                fields.set("name", if directory { "New folder" } else { "New file.txt" });
                self.asking = Some(Asking::New { directory });
            }
            "made" => {
                if let Some(Asking::New { directory }) = self.asking.take() {
                    let typed = fields.get("name").to_string();
                    self.make(&typed, directory);
                }
            }
            "cut" | "copy" => {
                if let Some(index) = index {
                    self.picked = Some(index);
                    let held = self.entries[index].name.clone();
                    self.hold(&held, name == "cut");
                }
            }
            "paste" => self.paste(),
            "permissions" => {
                if let Some(index) = index {
                    self.picked = Some(index);
                    self.permissions(index);
                }
            }
            "open-here" => {
                if let Some(id) = value["app"].as_str() {
                    self.open_here(id);
                }
            }
            "next" => self.picked = last.map(|last| self.picked.map_or(0, |picked| (picked + 1).min(last))),
            "previous" => self.picked = last.map(|_| self.picked.map_or(0, |picked| picked.saturating_sub(1))),
            "sort" => {
                if let Some(by) = value["by"].as_str().and_then(By::named) {
                    self.ascending = if by == self.by { !self.ascending } else { true };
                    self.by = by;
                    let was = self.picked.and_then(|picked| self.entries.get(picked)).map(|entry| entry.name.clone());
                    self.sort();
                    // The same entry stays picked, wherever it went.
                    self.picked = was.and_then(|name| self.entries.iter().position(|entry| entry.name == name));
                }
            }
            "up" => {
                if let Some(parent) = self.at.parent().map(Path::to_path_buf) {
                    self.go(parent, fields);
                }
            }
            "visit" => {
                if let Some(path) = value["path"].as_str().filter(|path| path.starts_with('/')) {
                    self.go(PathBuf::from(path), fields);
                }
            }
            "go" => {
                let typed = fields.get("path").trim();
                let to = if typed.starts_with('/') { PathBuf::from(typed) } else { self.at.join(typed) };
                self.go(to, fields);
            }
            "reread" => {
                self.reread();
            }
            _ => {}
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of its own with a few things in it, and a catalogue with
    /// two applications that open text and one that opens directories.
    fn explorer(name: &str) -> (Explorer, PathBuf) {
        let dir = std::env::temp_dir().join(format!("gexora-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (files, apps) = (dir.join("files"), dir.join("apps"));
        std::fs::create_dir_all(files.join("Music/Albums")).unwrap();
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(files.join("notes.txt"), "notes").unwrap();
        std::fs::write(files.join("other.txt"), "other").unwrap();
        std::fs::write(files.join("Music/Albums/one.flac"), "").unwrap();
        // The program has to be a file that is there, and this one is.
        let program = std::env::current_exe().unwrap();
        for (id, title, opens) in [("org.example.b", "Alpha", "text/*"), ("org.example.a", "Beta", "text/plain"), ("dev.peios.gexora", "Gexora", "inode/directory"), ("org.example.none", "None", "")] {
            let opens = if opens.is_empty() { String::new() } else { format!("opens = [\"{opens}\"]\n") };
            std::fs::write(apps.join(format!("{id}.toml")), format!("title = \"{title}\"\nprogram = \"{}\"\n{opens}", program.display())).unwrap();
        }
        let held = dir.join("held");
        (Explorer::with(files, Catalogue::at(apps), Choosing::Fixed(Defaults::default()), Holding::at(Some(&held))), dir)
    }

    /// Another window of the same session's, at `at`.
    fn another(dir: &Path, at: &str) -> Explorer {
        Explorer::with(dir.join("files").join(at), Catalogue::at(dir.join("apps")), Choosing::Fixed(Defaults::default()), Holding::at(Some(&dir.join("held"))))
    }

    /// Choices of what opens a type: the person's, and the machine's.
    fn chosen(person: &[(&str, &str)], machine: &[(&str, &str)]) -> Choosing {
        let of = |pairs: &[(&str, &str)]| pairs.iter().map(|(media, id)| (media.to_string(), id.to_string())).collect();
        Choosing::Fixed(Defaults { person: of(person), machine: of(machine) })
    }

    /// The labels of the menu `id` in what is shown.
    fn menu_labels(html: &str, id: &str) -> Vec<String> {
        let menu = html.split(&format!("<menu id=\"{id}\" hidden>")).nth(1).unwrap().split("</menu>").next().unwrap();
        menu.split("</button>").filter_map(|item| item.rsplit('>').next()).filter(|label| !label.is_empty()).map(String::from).collect()
    }

    fn shown(explorer: &Explorer) -> String {
        explorer.render(&Facts { views: 1, fields: &Fields::default() })
    }

    fn names(explorer: &Explorer) -> Vec<&str> {
        explorer.entries.iter().map(|entry| entry.name.as_str()).collect()
    }

    /// What comes with an event: the values an element was written with.
    fn said(values: &[(&str, &str)]) -> Value {
        values.iter().map(|(name, value)| (name.to_string(), value.to_string())).collect()
    }

    fn about(name: &str) -> Value {
        said(&[("name", name)])
    }

    #[test]
    fn the_candidates_are_what_opens_the_type_by_title() {
        let (explorer, dir) = explorer("candidates");
        let titles = |name: &str| explorer.candidates(&explorer.entries[explorer.named(name).unwrap()]).iter().map(|a| a.title.as_str()).collect::<Vec<_>>();
        assert_eq!(titles("notes.txt"), ["Alpha", "Beta"]);
        assert_eq!(titles("Music"), ["Gexora"]);
        let html = shown(&explorer);
        // One menu for the directory, one for the two files, and one for
        // where they all are.
        assert_eq!(html.matches("<menu id=").count(), 3);
        assert!(html.contains("fx-menu=\"menu-0\" fx-value-index=\"0\" fx-value-name=\"Music\""));
        assert!(html.contains("fx-menu=\"menu-1\" fx-value-index=\"1\" fx-value-name=\"notes.txt\""));
        assert!(html.contains("fx-menu=\"menu-1\" fx-value-index=\"2\" fx-value-name=\"other.txt\""));
        assert!(html.contains(&format!("fx-value-path=\"{}\"", dir.join("files/notes.txt").display())));
        let menu = html.split("<menu id=\"menu-1\" hidden>").nth(1).unwrap().split("</menu>").next().unwrap();
        let labels: Vec<&str> = menu.split("</button>").filter_map(|item| item.rsplit('>').next()).filter(|label| !label.is_empty()).collect();
        assert_eq!(labels, ["Open", "Open with Alpha", "Open with Beta", "Cut", "Copy", "Rename", "Copy path", "Permissions…", "Delete"]);
        assert!(menu.contains("fx-copy=\"path\""));
        // The directory's own menu is the listing's, and opens it with
        // what opens directories.
        assert!(html.contains("<div class=\"listing\" id=\"listing\" fx-menu=\"menu-here\""));
        let menu = html.split("<menu id=\"menu-here\" hidden>").nth(1).unwrap().split("</menu>").next().unwrap();
        let labels: Vec<&str> = menu.split("</button>").filter_map(|item| item.rsplit('>').next()).filter(|label| !label.is_empty()).collect();
        assert_eq!(labels, ["New folder", "New file", "Paste", "Open with Gexora"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn what_opens_a_type_by_default_is_the_persons_choice_else_the_machines() {
        fn titles(explorer: &Explorer, name: &str) -> Vec<String> {
            explorer.candidates(&explorer.entries[explorer.named(name).unwrap()]).iter().map(|a| a.title.clone()).collect()
        }
        let (mut explorer, dir) = explorer("defaults");
        // The machine's choice goes first, and opening uses it.
        explorer.choosing = chosen(&[], &[("text/plain", "org.example.a")]);
        explorer.reread();
        assert_eq!(titles(&explorer, "notes.txt"), ["Beta", "Alpha"]);
        assert_eq!(explorer.opening("text/plain").first().map(|a| a.id.as_str()), Some("org.example.a"));
        assert_eq!(menu_labels(&shown(&explorer), "menu-1"), ["Open", "Open with Beta", "Open with Alpha", "Cut", "Copy", "Rename", "Copy path", "Permissions…", "Delete"]);
        // The person's goes before it, even for every type of text.
        explorer.choosing = chosen(&[("text/*", "org.example.b")], &[("text/plain", "org.example.a")]);
        explorer.reread();
        assert_eq!(titles(&explorer, "notes.txt"), ["Alpha", "Beta"]);
        // A choice of something that does not open the type is passed over,
        // and so is one of something not installed, and the rule is the
        // titles' again.
        explorer.choosing = chosen(&[("text/plain", "dev.peios.gexora")], &[("text/*", "org.example.gone")]);
        explorer.reread();
        assert_eq!(titles(&explorer, "notes.txt"), ["Alpha", "Beta"]);
        assert_eq!(titles(&explorer, "Music"), ["Gexora"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_choices_are_read_again_when_a_file_is_opened() {
        let (mut explorer, dir) = explorer("chosen-now");
        let mut fields = Fields::default();
        assert!(explorer.defaults.person.is_empty());
        // Chosen elsewhere since the directory was read. Opening a file of
        // a type nothing opens starts nothing, and still reads them.
        explorer.choosing = chosen(&[("text/plain", "org.example.a")], &[]);
        explorer.event("visit", &said(&[("path", &dir.join("files/Music/Albums").to_string_lossy())]), &mut fields);
        explorer.choosing = chosen(&[("text/plain", "org.example.b")], &[]);
        explorer.event("open", &about("one.flac"), &mut fields);
        assert_eq!(explorer.defaults.person.get("text/plain").map(String::as_str), Some("org.example.b"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_files_rights_each_have_all_of_the_ones_after_them_that_they_can() {
        let rights = file_rights();
        let mask = |name: &str| rights.iter().find(|right| right.name == name).unwrap().mask;
        let has = |more: &str, less: &str| mask(more) & mask(less) == mask(less);
        assert!(rights.iter().all(|right| right.general && right.mask != 0));
        assert!(has("Full control", "Modify") && has("Modify", "Read & execute") && has("Read & execute", "Read") && has("Modify", "Write"));
        assert!(!has("Read", "Write") && !has("Modify", "Full control"));
        // As the kernel maps them, not as this guesses.
        assert_eq!(mask("Full control"), AccessMask::GENERIC_ALL.resolve_generic(&File::generic_mapping()).bits());
    }

    #[test]
    fn a_directory_is_opened_only_with_what_opens_directories() {
        let (mut explorer, dir) = explorer("here");
        let mut fields = Fields::default();
        explorer.event("open-here", &said(&[("app", "org.example.a")]), &mut fields);
        assert_eq!(explorer.trouble.as_deref(), Some("This directory cannot be opened with that."));
        explorer.event("open-here", &said(&[("app", "org.example.nowhere")]), &mut fields);
        assert_eq!(explorer.trouble.as_deref(), Some("This directory cannot be opened with that."));
        // What says nothing of its type and does not read as text is
        // opened by nothing.
        std::fs::write(dir.join("files/thing"), b"\x7fELF\0\0").unwrap();
        explorer.event("reread", &Value::Null, &mut fields);
        explorer.event("open", &about("thing"), &mut fields);
        assert_eq!(explorer.trouble.as_deref(), Some("Nothing installed says it opens files like this."));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_folder_or_a_file_is_made_by_the_name_typed_and_over_nothing() {
        let (mut explorer, dir) = explorer("making");
        let mut fields = Fields::default();
        explorer.event("new-folder", &Value::Null, &mut fields);
        assert_eq!(fields.get("name"), "New folder");
        let html = shown(&explorer);
        assert!(html.contains("<ul class=\"entries\"><li><form id=\"making\" class=\"directory renaming\" fx-submit=\"made\">"));
        assert!(!html.contains("fx-key=\"Enter\""));
        fields.set("name", "Pictures");
        explorer.event("made", &Value::Null, &mut fields);
        assert!(dir.join("files/Pictures").is_dir());
        assert_eq!(names(&explorer), ["Music", "Pictures", "notes.txt", "other.txt"]);
        assert_eq!(explorer.picked, explorer.named("Pictures"));

        explorer.event("new-file", &Value::Null, &mut fields);
        assert_eq!(fields.get("name"), "New file.txt");
        fields.set("name", "notes.txt");
        explorer.event("made", &Value::Null, &mut fields);
        assert_eq!(explorer.trouble.as_deref(), Some("There is already something called notes.txt here."));
        assert_eq!(std::fs::read_to_string(dir.join("files/notes.txt")).unwrap(), "notes");
        explorer.event("new-file", &Value::Null, &mut fields);
        fields.set("name", "a/b");
        explorer.event("made", &Value::Null, &mut fields);
        assert_eq!(explorer.trouble.as_deref(), Some("That is not a name a file can have."));
        explorer.event("new-file", &Value::Null, &mut fields);
        fields.set("name", "list.txt");
        explorer.event("made", &Value::Null, &mut fields);
        assert_eq!(std::fs::read_to_string(dir.join("files/list.txt")).unwrap(), "");
        // Asked and then not: nothing is made.
        explorer.event("new-folder", &Value::Null, &mut fields);
        explorer.event("keep", &Value::Null, &mut fields);
        fields.set("name", "Never");
        explorer.event("made", &Value::Null, &mut fields);
        assert!(!dir.join("files/Never").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn what_is_copied_in_one_window_is_pasted_in_another_and_over_nothing() {
        let (mut explorer, dir) = explorer("copying");
        let mut fields = Fields::default();
        explorer.event("paste", &Value::Null, &mut fields);
        assert_eq!(explorer.trouble.as_deref(), Some("Nothing has been cut or copied."));
        explorer.event("copy", &about("notes.txt"), &mut fields);
        assert!(!shown(&explorer).contains(" cut\""));
        // Where it came from, it is a copy by name.
        explorer.event("paste", &Value::Null, &mut fields);
        assert_eq!(names(&explorer), ["Music", "notes (copy).txt", "notes.txt", "other.txt"]);
        assert_eq!(explorer.picked, explorer.named("notes (copy).txt"));
        // Still held, for another window to paste.
        let mut other = another(&dir, "Music");
        other.event("paste", &Value::Null, &mut fields);
        other.event("paste", &Value::Null, &mut fields);
        assert_eq!(names(&other), ["Albums", "notes (copy).txt", "notes.txt"]);
        assert_eq!(std::fs::read_to_string(dir.join("files/Music/notes.txt")).unwrap(), "notes");

        // A directory goes with everything in it, and not into itself.
        explorer.event("copy", &about("Music"), &mut fields);
        other.event("paste", &Value::Null, &mut fields);
        assert_eq!(other.trouble.as_deref(), Some("Music cannot be put inside itself."));
        let mut deeper = another(&dir, "Music/Albums");
        deeper.event("paste", &Value::Null, &mut fields);
        assert_eq!(deeper.trouble.as_deref(), Some("Music cannot be put inside itself."));
        explorer.event("paste", &Value::Null, &mut fields);
        assert_eq!(explorer.trouble, None);
        assert!(dir.join("files/Music (copy)/Albums/one.flac").exists());

        // What has gone since it was copied is said to have.
        explorer.event("copy", &about("other.txt"), &mut fields);
        std::fs::remove_file(dir.join("files/other.txt")).unwrap();
        other.event("paste", &Value::Null, &mut fields);
        assert_eq!(other.trouble.as_deref(), Some("other.txt is no longer where it was."));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn what_is_cut_is_moved_once_and_onto_nothing() {
        let (mut explorer, dir) = explorer("cutting");
        let mut fields = Fields::default();
        std::fs::write(dir.join("files/Music/notes.txt"), "theirs").unwrap();
        explorer.event("cut", &about("notes.txt"), &mut fields);
        assert!(shown(&explorer).contains("class=\"file cut\""));
        // Onto a name that is taken, it stays where it is and stays held.
        let mut other = another(&dir, "Music");
        other.event("paste", &Value::Null, &mut fields);
        assert_eq!(other.trouble.as_deref(), Some("There is already something called notes.txt here."));
        assert_eq!(std::fs::read_to_string(dir.join("files/Music/notes.txt")).unwrap(), "theirs");
        assert_eq!(std::fs::read_to_string(dir.join("files/notes.txt")).unwrap(), "notes");
        let mut deeper = another(&dir, "Music/Albums");
        deeper.event("paste", &Value::Null, &mut fields);
        assert_eq!(deeper.trouble, None);
        assert_eq!(names(&deeper), ["notes.txt", "one.flac"]);
        assert!(!dir.join("files/notes.txt").exists());
        // Pasted once: there is nothing held any more.
        deeper.event("paste", &Value::Null, &mut fields);
        assert_eq!(deeper.trouble.as_deref(), Some("Nothing has been cut or copied."));
        // Cut and pasted where it is, it stays, and is let go.
        explorer.event("reread", &Value::Null, &mut fields);
        explorer.event("cut", &about("other.txt"), &mut fields);
        explorer.event("paste", &Value::Null, &mut fields);
        assert_eq!((explorer.trouble.as_deref(), names(&explorer)), (None, vec!["Music", "other.txt"]));
        assert!(!shown(&explorer).contains(" cut\""));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn only_a_candidate_opens_a_file_and_nothing_opens_what_nothing_says_it_opens() {
        let (mut explorer, dir) = explorer("opening");
        let mut fields = Fields::default();
        // Gexora does not open text, whatever a page says.
        explorer.event("open-with", &said(&[("name", "notes.txt"), ("app", "dev.peios.gexora")]), &mut fields);
        assert_eq!(explorer.trouble.as_deref(), Some("notes.txt cannot be opened with that."));
        explorer.event("visit", &said(&[("path", &dir.join("files/Music/Albums").to_string_lossy())]), &mut fields);
        explorer.event("open", &about("one.flac"), &mut fields);
        assert_eq!(explorer.trouble.as_deref(), Some("Nothing installed says it opens audio/flac files."));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_new_name_is_typed_where_the_name_was_and_nothing_is_written_over() {
        let (mut explorer, dir) = explorer("renaming");
        let mut fields = Fields::default();
        explorer.event("rename", &about("notes.txt"), &mut fields);
        assert_eq!(fields.get("name"), "notes.txt");
        let html = shown(&explorer);
        assert!(html.contains("<form id=\"renaming-1\" class=\"file renaming\" fx-submit=\"renamed\""));
        assert!(html.contains("<input name=\"name\" fx-autofocus"));
        // Enter is the form's while a name is being typed, and Escape is the way out.
        assert!(!html.contains("fx-key=\"Enter\""));
        assert!(html.contains("fx-key=\"Escape\" fx-click=\"keep\""));
        // A name something has already is refused, and both are as they were.
        fields.set("name", "other.txt");
        explorer.event("renamed", &Value::Null, &mut fields);
        assert_eq!(explorer.trouble.as_deref(), Some("There is already something called other.txt here."));
        assert_eq!(std::fs::read_to_string(dir.join("files/other.txt")).unwrap(), "other");
        assert_eq!(names(&explorer), ["Music", "notes.txt", "other.txt"]);
        // So is what is no name.
        for not_a_name in ["", "  ", "..", "a/b"] {
            explorer.event("rename", &about("notes.txt"), &mut fields);
            fields.set("name", not_a_name);
            explorer.event("renamed", &Value::Null, &mut fields);
            assert_eq!(explorer.trouble.as_deref(), Some("That is not a name a file can have."), "{not_a_name:?}");
        }
        explorer.event("rename", &about("notes.txt"), &mut fields);
        fields.set("name", " words.txt ");
        explorer.event("renamed", &Value::Null, &mut fields);
        assert_eq!(explorer.trouble, None);
        assert_eq!(names(&explorer), ["Music", "other.txt", "words.txt"]);
        assert_eq!(explorer.picked, explorer.named("words.txt"));
        assert_eq!(std::fs::read_to_string(dir.join("files/words.txt")).unwrap(), "notes");
        // Asked to rename and then not: nothing happens.
        explorer.event("rename", &about("words.txt"), &mut fields);
        explorer.event("keep", &Value::Null, &mut fields);
        fields.set("name", "never.txt");
        explorer.event("renamed", &Value::Null, &mut fields);
        assert_eq!(names(&explorer), ["Music", "other.txt", "words.txt"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn deleting_asks_first_and_deletes_only_what_was_asked_about() {
        let (mut explorer, dir) = explorer("deleting");
        let mut fields = Fields::default();
        // Nothing is deleted that was not asked about.
        explorer.event("deleted", &about("notes.txt"), &mut fields);
        assert_eq!(names(&explorer), ["Music", "notes.txt", "other.txt"]);
        explorer.event("delete", &about("notes.txt"), &mut fields);
        let html = shown(&explorer);
        assert!(html.contains("<p>Delete notes.txt? There is nowhere to get it back from.</p>"));
        // Enter presses whichever button the keyboard is on, which starts on the way out.
        assert!(!html.contains("fx-key=\"Enter\""));
        assert!(html.contains("fx-click=\"keep\" fx-key=\"Escape\" fx-autofocus"));
        // Asked about one and told of another: neither goes.
        explorer.event("deleted", &about("other.txt"), &mut fields);
        assert_eq!(names(&explorer), ["Music", "notes.txt", "other.txt"]);
        assert!(explorer.asking.is_none());
        explorer.event("delete", &about("notes.txt"), &mut fields);
        explorer.event("keep", &Value::Null, &mut fields);
        explorer.event("deleted", &about("notes.txt"), &mut fields);
        assert_eq!(names(&explorer), ["Music", "notes.txt", "other.txt"]);
        explorer.event("delete", &about("notes.txt"), &mut fields);
        explorer.event("deleted", &about("notes.txt"), &mut fields);
        assert_eq!(names(&explorer), ["Music", "other.txt"]);
        // A directory goes with everything in it, and says so first.
        explorer.event("delete", &about("Music"), &mut fields);
        assert!(shown(&explorer).contains("<p>Delete Music and everything in it? There is nowhere to get it back from.</p>"));
        explorer.event("deleted", &about("Music"), &mut fields);
        assert_eq!(names(&explorer), ["other.txt"]);
        assert!(!dir.join("files/Music").exists());
        // A link is deleted as the link it is, and what it links to stays.
        std::os::unix::fs::symlink(dir.join("apps"), dir.join("files/link")).unwrap();
        explorer.event("reread", &Value::Null, &mut fields);
        explorer.event("delete", &about("link"), &mut fields);
        explorer.event("deleted", &about("link"), &mut fields);
        assert_eq!(names(&explorer), ["other.txt"]);
        assert!(dir.join("apps/dev.peios.gexora.toml").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

fn main() {
    let mut app = match App::connect() {
        Ok(app) => app,
        Err(e) => {
            eprintln!("gexora: no desktop to open on: {e}");
            std::process::exit(1);
        }
    };
    app.stylesheet("/gexora.css", include_str!("gexora.css"));
    // Where asked, or at home, or at the root if there is no home to be at.
    let at = std::env::args().nth(1).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(PathBuf::from));
    let at = at.filter(|at| at.is_dir()).unwrap_or_else(|| PathBuf::from("/"));
    let window = app.live("Gexora", Explorer::new(at));
    let aside = std::sync::Arc::downgrade(&window);
    window.update(|explorer, fields| {
        fields.set("path", &explorer.at.to_string_lossy());
        explorer.window = aside;
    });
    if let Err(e) = app.run() {
        eprintln!("gexora: {e}");
        std::process::exit(1);
    }
}
