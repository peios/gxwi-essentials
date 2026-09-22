//! Gexora — a file explorer: one window showing a directory, with a way into
//! each thing in it.
//!
//! It is an app like any other, drawn by this process for whoever is looking.
//! Where it is looking, and which entry is picked, are the window's, so two
//! devices showing the one window see the same directory: another directory
//! at the same time is another window.
//!
//! Every entry is drawn with its icon as PGSS Icons finds it, which is what
//! this is first for: a directory full of things is the icon rules tried all
//! at once.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use libgxwi::icons::{self, Themes};
use libgxwi::{App, Facts, Fields, Live, Value, escape};

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
}

struct Explorer {
    themes: Themes,
    at: PathBuf,
    /// The directory as last read, and when. Read again on arrival and on
    /// asking, not on every render: a render happens on every keystroke.
    entries: Vec<Entry>,
    read: Option<SystemTime>,
    /// How many were not drawn, past `MOST`.
    unshown: usize,
    /// Why the directory could not be read, if it could not.
    trouble: Option<String>,
    picked: usize,
}

impl Explorer {
    fn new(at: PathBuf) -> Explorer {
        let mut explorer = Explorer {
            themes: Themes::active(),
            at,
            entries: Vec::new(),
            read: None,
            unshown: 0,
            trouble: None,
            picked: 0,
        };
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
        let mut entries: Vec<Entry> = listing
            .filter_map(|entry| entry.map_err(|e| failed = Some(e)).ok())
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let path = entry.path();
                // Through a link, as the thing opened would be.
                let metadata = std::fs::metadata(&path).ok();
                let directory = metadata.as_ref().is_some_and(|m| m.is_dir());
                let program = metadata.as_ref().is_some_and(|m| m.is_file()) && may_run(&path);
                Some(Entry { icon: icons::identify(&path, &self.themes), name, directory, program })
            })
            .collect();
        // Directories first, then by name without regard to case, so a
        // listing reads the same wherever it was made.
        entries.sort_by_cached_key(|entry| (!entry.directory, entry.name.to_lowercase(), entry.name.clone()));
        self.unshown = entries.len().saturating_sub(MOST);
        entries.truncate(MOST);
        self.trouble = failed.map(|e| match (e.kind(), entries.is_empty()) {
            (std::io::ErrorKind::PermissionDenied, true) => "You are not allowed to see what is in here.".into(),
            _ => format!("Not everything in here could be read: {e}."),
        });
        self.entries = entries;
        self.read = Some(SystemTime::now());
        self.picked = self.picked.min(self.entries.len().saturating_sub(1));
        true
    }

    /// Goes to `to`, if it is a directory that can be opened.
    fn go(&mut self, to: PathBuf, fields: &mut Fields) {
        let was = std::mem::replace(&mut self.at, to);
        self.picked = 0;
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
}

/// Whether this process may run the file: the system's word, asked the way
/// an exec would ask it.
fn may_run(path: &Path) -> bool {
    let Ok(cpath) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else { return false };
    // SAFETY: a NUL-terminated string, live for the call.
    unsafe { libc::access(cpath.as_ptr(), libc::X_OK) == 0 }
}

impl Live for Explorer {
    fn render(&self, _: &Facts) -> String {
        let entries: String = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let picked = index == self.picked;
                format!(
                    "<li><button type=\"button\" id=\"entry-{index}\" class=\"{class}\" fx-click=\"pick\" fx-dblclick=\"open\" \
                     fx-value-index=\"{index}\" aria-selected=\"{picked}\">\
                     <img src=\"{icon}\" alt=\"\"><span>{name}</span></button></li>",
                    class = if entry.directory { "directory" } else { "file" },
                    icon = escape(&icons::href(&entry.icon)),
                    name = escape(&entry.name),
                )
            })
            .collect();
        let trouble = self.trouble.as_ref().map(|why| format!("<p class=\"trouble\">{}</p>", escape(why))).unwrap_or_default();
        let more = if self.unshown > 0 { format!("<p class=\"more\">and {} more, not shown</p>", self.unshown) } else { String::new() };
        let empty = if self.entries.is_empty() && self.trouble.is_none() { "<p class=\"more\">Nothing in here.</p>" } else { "" };
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
             {trouble}<ul class=\"entries\">{entries}</ul>{empty}{more}",
            picked = self.picked,
        )
    }

    fn event(&mut self, name: &str, value: &Value, fields: &mut Fields) {
        let index = value["index"].as_str().and_then(|index| index.parse().ok());
        match name {
            "pick" => self.picked = index.unwrap_or(self.picked).min(self.entries.len().saturating_sub(1)),
            "open" => {
                if let Some(index) = index {
                    self.open(index, fields);
                }
            }
            "next" => self.picked = (self.picked + 1).min(self.entries.len().saturating_sub(1)),
            "previous" => self.picked = self.picked.min(self.entries.len().saturating_sub(1)).saturating_sub(1),
            "up" => {
                if let Some(parent) = self.at.parent().map(Path::to_path_buf) {
                    self.go(parent, fields);
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
