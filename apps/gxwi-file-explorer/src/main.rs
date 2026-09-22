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

use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
}

impl Explorer {
    fn new(at: PathBuf) -> Explorer {
        let mut explorer = Explorer {
            themes: Themes::active(),
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
        true
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
            // Started as this process was, on the same desktop, and left to
            // itself. Reaped when it ends, so that it does not linger.
            match std::process::Command::new(&path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(mut child) => {
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                }
                Err(e) => self.trouble = Some(format!("{} could not be started: {e}.", entry.name)),
            }
        }
        // Anything else waits for something that says what opens it.
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
        let entries: String = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                format!(
                    "<li><button type=\"button\" id=\"entry-{index}\" class=\"{class}\" fx-click=\"pick\" fx-dblclick=\"open\" \
                     fx-value-index=\"{index}\" aria-selected=\"{picked}\">\
                     <img src=\"{icon}\" alt=\"\"><span class=\"name\">{name}</span>\
                     <span class=\"modified\">{modified}</span><span class=\"kind\">{kind}</span><span class=\"size\">{size}</span>\
                     </button></li>",
                    class = if entry.directory { "directory" } else { "file" },
                    picked = self.picked == Some(index),
                    icon = escape(&icons::href(&entry.icon)),
                    name = escape(&entry.name),
                    modified = show::when(entry.modified, now),
                    kind = entry.kind(),
                    size = if entry.directory { "—".into() } else { show::size(entry.size) },
                )
            })
            .collect();
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
             <button type=\"button\" fx-key=\"Enter\" fx-click=\"open\" fx-value-index=\"{picked}\"></button>\
             <button type=\"button\" fx-key=\"ArrowDown\" fx-click=\"next\"></button>\
             <button type=\"button\" fx-key=\"ArrowUp\" fx-click=\"previous\"></button>\
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
             {trouble}<ul class=\"entries\">{entries}</ul>{empty}{more}\
             </div>\
             {details}\
             </div>\
             <footer class=\"status\"><span>{counted}</span><span>{space}</span></footer>",
            picked = self.picked.map(|picked| picked.to_string()).unwrap_or_default(),
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
        match name {
            "pick" => self.picked = index.or(self.picked),
            "open" => {
                if let Some(index) = index {
                    self.open(index, fields);
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
