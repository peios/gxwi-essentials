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
//! catalogue whose `opens` the type matches, in the order of their titles,
//! and opening a file uses the first of them. The standard has no default
//! application for a type and leaves the rule to whoever opens, and this is
//! the rule here. An entry's menu offers every candidate by name. Gexora is
//! no part of the shell, so it starts what it opens itself, as a child of
//! its own on the desktop it is on.
//!
//! AN ENTRY'S MENU also renames it, copies its path, and deletes it. There
//! is nowhere for what is deleted to go, so deleting asks first, and a
//! directory goes with everything in it. A new name that is already
//! something's is refused: nothing is ever written over.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use libgxwi::apps::{Application, Catalogue};
use libgxwi::icons::{self, Themes};
use libgxwi::{App, Facts, Fields, Live, Value, escape};

mod show;

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
    asking: Option<Asking>,
}

impl Explorer {
    fn new(at: PathBuf) -> Explorer {
        Explorer::with(at, Catalogue::system())
    }

    fn with(at: PathBuf, catalogue: Catalogue) -> Explorer {
        let mut explorer = Explorer {
            themes: Themes::active(),
            catalogue,
            openers: Vec::new(),
            asking: None,
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
        // What was being asked about may have gone.
        self.asking = self.asking.take().filter(|asking| {
            let (Asking::Rename(name) | Asking::Delete(name)) = asking;
            self.entries.iter().any(|entry| &entry.name == name)
        });
        true
    }

    /// The applications that are candidates to open `entry`, first the one
    /// that opening it uses.
    fn candidates(&self, entry: &Entry) -> Vec<&Application> {
        let Some(media) = entry.opened_as() else { return Vec::new() };
        self.openers.iter().filter(|application| application.opens(media)).collect()
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
        let Some(entry) = self.entries.get(index) else { return };
        let path = self.at.join(&entry.name);
        if entry.directory {
            self.go(path, fields);
        } else if entry.program {
            if let Err(e) = start(&path, &[]) {
                self.trouble = Some(format!("{} could not be started: {e}.", entry.name));
            }
        } else {
            match self.candidates(entry).first().map(|application| application.id.clone()) {
                Some(id) => self.open_with(index, &id, fields),
                None => self.trouble = Some(format!("Nothing installed says it opens {}.", entry.opened_as().map_or("files like this".into(), |media| format!("{media} files")))),
            }
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
        if to.is_empty() || to == "." || to == ".." || to.contains('/') || to.contains('\0') {
            self.trouble = Some("That is not a name a file can have.".into());
            return;
        }
        let target = self.at.join(to);
        // Asked of the thing itself and not what it may link to: a link
        // that goes nowhere still has the name.
        if std::fs::symlink_metadata(&target).is_ok() {
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
        let path = self.at.join(name);
        let removed = match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(&path),
            Ok(_) => std::fs::remove_file(&path),
            Err(e) => Err(e),
        };
        self.reread();
        if let Err(e) = removed {
            self.trouble = Some(format!("{name} could not be deleted: {}.", why(&e)));
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
        format!(
            "<aside class=\"details\" aria-label=\"Details\">\
             <img src=\"{icon}\" alt=\"\"><h2>{name}</h2>\
             <dl>{kind}{size}{modified}{created}</dl>\
             </aside>",
            icon = escape(&icons::href(&entry.icon)),
            name = escape(&entry.name),
            kind = row("Type", entry.kind().into()),
            size = row("Size", size),
            modified = row("Modified", show::when_long(entry.modified)),
            created = row("Created", show::when_long(entry.created)),
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
                let class = if entry.directory { "directory" } else { "file" };
                let rest = format!(
                    "<span class=\"modified\">{modified}</span><span class=\"kind\">{kind}</span><span class=\"size\">{size}</span>",
                    modified = show::when(entry.modified, now),
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
                     <li><button type=\"button\" fx-click=\"rename\">Rename</button></li>\
                     <li><button type=\"button\" fx-copy=\"path\">Copy path</button></li><hr>\
                     <li><button type=\"button\" fx-click=\"delete\">Delete</button></li>\
                     </menu>"
                )
            })
            .collect();
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
            Some(Asking::Rename(_)) => "<button type=\"button\" fx-key=\"Escape\" fx-click=\"keep\"></button>".to_string(),
            Some(Asking::Delete(_)) => String::new(),
            None => format!(
                "<button type=\"button\" fx-key=\"Enter\" fx-click=\"open\" fx-value-index=\"{picked}\"></button>\
                 <button type=\"button\" fx-key=\"ArrowDown\" fx-click=\"next\"></button>\
                 <button type=\"button\" fx-key=\"ArrowUp\" fx-click=\"previous\"></button>\
                 <button type=\"button\" fx-key=\"F2\" fx-click=\"rename\" fx-value-index=\"{picked}\"></button>\
                 <button type=\"button\" fx-key=\"Delete\" fx-click=\"delete\" fx-value-index=\"{picked}\"></button>",
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
        let empty = if self.entries.is_empty() && self.trouble.is_none() { "<p class=\"more\">Nothing in here.</p>" } else { "" };
        let counted = match self.picked.and_then(|picked| self.entries.get(picked)) {
            Some(entry) if !entry.directory => format!("{} items · 1 selected · {}", self.entries.len(), show::size(entry.size)),
            Some(_) => format!("{} items · 1 selected", self.entries.len()),
            None => format!("{} items", self.entries.len()),
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
             <div class=\"listing\" id=\"listing\" fx-columns=\"16px minmax(0, 1fr) 140px 130px 80px\">\
             <div class=\"head\"><span></span>{name}{modified}{kind}{size}</div>\
             {trouble}{asking}<ul class=\"entries\">{entries}</ul>{empty}{more}\
             </div>\
             {details}\
             </div>\
             <footer class=\"status\"><span>{counted}</span><span>{space}</span></footer>{menus}",
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
        (Explorer::with(files, Catalogue::at(apps)), dir)
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
        // One menu for the directory and one for the two files.
        assert_eq!(html.matches("<menu id=").count(), 2);
        assert!(html.contains("fx-menu=\"menu-0\" fx-value-index=\"0\" fx-value-name=\"Music\""));
        assert!(html.contains("fx-menu=\"menu-1\" fx-value-index=\"1\" fx-value-name=\"notes.txt\""));
        assert!(html.contains("fx-menu=\"menu-1\" fx-value-index=\"2\" fx-value-name=\"other.txt\""));
        assert!(html.contains(&format!("fx-value-path=\"{}\"", dir.join("files/notes.txt").display())));
        let menu = html.split("<menu id=\"menu-1\" hidden>").nth(1).unwrap().split("</menu>").next().unwrap();
        let labels: Vec<&str> = menu.split("</button>").filter_map(|item| item.rsplit('>').next()).filter(|label| !label.is_empty()).collect();
        assert_eq!(labels, ["Open", "Open with Alpha", "Open with Beta", "Rename", "Copy path", "Delete"]);
        assert!(menu.contains("fx-copy=\"path\""));
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
    window.update(|explorer, fields| fields.set("path", &explorer.at.to_string_lossy()));
    if let Err(e) = app.run() {
        eprintln!("gexora: {e}");
        std::process::exit(1);
    }
}
