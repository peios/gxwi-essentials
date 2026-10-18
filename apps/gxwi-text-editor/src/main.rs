//! gxwi-text-editor: an editor for files of text, on a GXWI desktop.
//!
//! One window, one file: the file named after the program's name, or a new
//! one with no name yet. Another file at the same time is another window.
//!
//! It is an app like any other, drawn by this process for whoever is
//! looking. The text is a field of the window's, so what has been typed is
//! the same on every device looking at it, and is still there when a
//! browser tab is closed and opened again. It is lost only by the window
//! being closed, which asks first when there is something to lose.
//!
//! SAVING writes the file where it is (`file`). Saving under another name
//! makes a new file and never writes over one that is there.
//!
//! WHAT SOMETHING ELSE DOES TO THE FILE is noticed, by looking at it every
//! so often. A file changed underneath text that has not been is read
//! again, and nothing is said. Changed underneath text that has, it is for
//! the person to say which is kept; and one that has gone is said to have,
//! and is put back by saving.
//!
//! What is not text, or is more than the editor holds, is not shown at all.

use std::path::{Path, PathBuf};
use std::time::Duration;

use libgxwi::{App, Closer, Facts, Fields, Live, Value, escape};

mod file;

use file::{Endings, Seen};

// What this program looks like, to whatever lists it. The icon itself is
// `gxwi-text-editor.svg` at the repo root, installed as the base theme's.
libgxwi::icon!(b"dev.peios.gxwi-text-editor");

/// How often the file is looked at for what something else did to it.
const LOOKS: Duration = Duration::from_secs(2);

/// What is being asked of the person before anything more is done.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Asking {
    /// Where to save, typed into a field. `closing` is whether the window
    /// goes once it is saved.
    Path { closing: bool },
    /// What to do with what has not been saved, the window being closed.
    Closing,
    /// Which is kept, the file having been changed by something else under
    /// text that has been changed here.
    Changed,
}

struct Editor {
    path: Option<PathBuf>,
    /// The text as the file has it, as far as is known.
    saved: String,
    endings: Endings,
    /// The file as it was when last read or written, or `None` for one
    /// that is not there.
    seen: Option<Seen>,
    /// Whether the file was there and has gone.
    gone: bool,
    /// Why the file is not shown, if it is not.
    refused: Option<String>,
    /// Whether the file may be written, or made.
    writable: bool,
    trouble: Option<String>,
    asking: Option<Asking>,
    /// The window, to call it something else and to let it go.
    window: Option<Closer>,
}

impl Editor {
    /// An editor on the file at `path`, there or not, or on a new one with
    /// no name. What the field is to hold comes back with it.
    fn on(path: Option<PathBuf>) -> (Editor, String) {
        let mut editor = Editor {
            writable: path.as_deref().is_none_or(file::may_write),
            path,
            saved: String::new(),
            endings: Endings::Feed,
            seen: None,
            gone: false,
            refused: None,
            trouble: None,
            asking: None,
            window: None,
        };
        let text = editor.read().unwrap_or_default();
        (editor, text)
    }

    /// Reads the file, if there is one. What it holds, unless it is not
    /// there or is not shown.
    fn read(&mut self) -> Option<String> {
        let path = self.path.as_deref()?;
        self.gone = false;
        self.refused = None;
        if let Ok(None) = file::seen(path) {
            // Named and not there yet: saving makes it.
            self.seen = None;
            self.saved.clear();
            return None;
        }
        match file::read(path) {
            Ok(read) => {
                self.saved = read.text.clone();
                self.endings = read.endings;
                self.seen = Some(read.seen);
                Some(read.text)
            }
            Err(refused) => {
                self.refused = Some(refused.to_string());
                self.seen = file::seen(path).ok().flatten();
                None
            }
        }
    }

    /// What the window is called: the file's name.
    fn name(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_name)
            .map_or("Untitled".into(), |name| {
                name.to_string_lossy().into_owned()
            })
    }

    /// Whether there is anything that saving would change.
    fn unsaved(&self, fields: &Fields) -> bool {
        self.refused.is_none() && (self.gone || fields.get("text") != self.saved)
    }

    /// Whether the file is other than it was when last read or written.
    fn differs(&self) -> bool {
        let Some(path) = self.path.as_deref() else {
            return false;
        };
        file::seen(path).is_ok_and(|now| now != self.seen)
    }

    /// Takes in what something else did to the file.
    fn look(&mut self, fields: &mut Fields) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let Ok(now) = file::seen(&path) else { return };
        if now == self.seen {
            return;
        }
        let unsaved = self.unsaved(fields);
        if now.is_none() {
            self.seen = None;
            self.gone = true;
            self.trouble = Some(format!(
                "{} is no longer there. Saving puts it back.",
                self.name()
            ));
        } else if unsaved && self.refused.is_none() {
            // Asked once for each change, however long the answer takes.
            self.seen = now;
            self.gone = false;
            self.asking = Some(Asking::Changed);
        } else {
            self.load(fields);
        }
    }

    /// Reads the file again over whatever is in the editor.
    fn load(&mut self, fields: &mut Fields) {
        if let Some(text) = self.read() {
            fields.set("text", &text);
        }
        self.writable = self.path.as_deref().is_none_or(file::may_write);
    }

    /// Saves to the file. Whether it was saved: with no file to save to the
    /// person is asked for one, and with the file changed by something else
    /// since it was read, which of the two is kept.
    fn save(&mut self, fields: &mut Fields) -> bool {
        let Some(path) = self.path.clone() else {
            self.ask_where(fields, false);
            return false;
        };
        if self.refused.is_some() {
            return false;
        }
        if self.seen.is_some() && self.differs() {
            self.look(fields);
            return false;
        }
        // One that was not there is made, and is not written over
        // something that has appeared there since.
        match file::write(&path, fields.get("text"), self.endings, self.seen.is_none()) {
            Ok(seen) => {
                self.saved = fields.get("text").to_string();
                self.seen = Some(seen);
                self.gone = false;
                if self.endings == Endings::Mixed {
                    self.endings = Endings::Feed;
                }
                true
            }
            Err(e) => {
                self.trouble = Some(format!("{} could not be saved: {}.", self.name(), why(&e)));
                false
            }
        }
    }

    /// Asks where to save, beginning from where the file is, or from home.
    fn ask_where(&mut self, fields: &mut Fields, closing: bool) {
        let suggested = match &self.path {
            Some(path) => path.clone(),
            None => home().join("untitled.txt"),
        };
        fields.set("path", &suggested.to_string_lossy());
        self.asking = Some(Asking::Path { closing });
    }

    /// Saves to a new file at the path typed, and is an editor on that file
    /// from then on. Whether it was saved.
    fn save_as(&mut self, typed: &str, fields: &mut Fields) -> bool {
        let typed = typed.trim();
        if typed.is_empty() || typed.contains('\0') || typed.ends_with('/') {
            self.trouble = Some("That is not a name a file can have.".into());
            return false;
        }
        let beside = self
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(home);
        let to = if typed.starts_with('/') {
            PathBuf::from(typed)
        } else {
            beside.join(typed)
        };
        match file::write(&to, fields.get("text"), self.endings, true) {
            Ok(seen) => {
                self.saved = fields.get("text").to_string();
                self.seen = Some(seen);
                self.gone = false;
                self.refused = None;
                self.writable = true;
                self.path = Some(to);
                if let Some(window) = &self.window {
                    window.retitle(&self.name());
                }
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                self.trouble = Some(format!(
                    "There is already something at {}. Nothing is written over from here.",
                    to.display()
                ));
                false
            }
            Err(e) => {
                self.trouble = Some(format!("{} could not be saved: {}.", to.display(), why(&e)));
                false
            }
        }
    }

    fn close(&self) {
        if let Some(window) = &self.window {
            window.close();
        }
    }

    /// What is asked, as a bar above the text.
    fn asked(&self) -> String {
        let name = escape(&self.name());
        match self.asking {
            None => String::new(),
            Some(Asking::Path { .. }) => "<form id=\"asking\" class=\"asking\" fx-submit=\"saved-as\">\
                 <label for=\"path\">Save as</label>\
                 <input id=\"path\" name=\"path\" fx-autofocus autocomplete=\"off\" spellcheck=\"false\">\
                 <button type=\"submit\">Save</button>\
                 <button type=\"button\" fx-click=\"cancel\" fx-key=\"Escape\">Cancel</button>\
                 </form>"
                .into(),
            Some(Asking::Closing) => format!(
                "<div id=\"asking\" class=\"asking\" role=\"alertdialog\" aria-label=\"Unsaved changes\">\
                 <p>{name} has changes that are not saved.</p>\
                 <button type=\"button\" fx-click=\"close-saved\" fx-autofocus>Save</button>\
                 <button type=\"button\" class=\"danger\" fx-click=\"close-discarded\">Discard</button>\
                 <button type=\"button\" fx-click=\"cancel\" fx-key=\"Escape\">Cancel</button>\
                 </div>"
            ),
            Some(Asking::Changed) => format!(
                "<div id=\"asking\" class=\"asking\" role=\"alertdialog\" aria-label=\"Changed by something else\">\
                 <p>{name} was changed by something else, and has changes here that are not saved.</p>\
                 <button type=\"button\" fx-click=\"keep\" fx-key=\"Escape\" fx-autofocus>Keep what is here</button>\
                 <button type=\"button\" class=\"danger\" fx-click=\"load\">Load the file</button>\
                 </div>"
            ),
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_dir())
        .unwrap_or_else(|| "/".into())
}

/// Why something could not be done, for the person to read.
fn why(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => "you are not allowed to".into(),
        std::io::ErrorKind::NotFound => "there is no such directory".into(),
        _ => error.to_string(),
    }
}

/// A size in bytes, to a few figures.
fn size(bytes: usize) -> String {
    match bytes {
        0..1000 => format!("{bytes} B"),
        1000..10_000 => format!("{:.1} kB", bytes as f64 / 1000.0),
        10_000..1_000_000 => format!("{:.0} kB", bytes as f64 / 1000.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_000_000.0),
    }
}

impl Live for Editor {
    fn render(&self, facts: &Facts) -> String {
        let text = facts.fields.get("text");
        let unsaved = self.unsaved(facts.fields);
        let state = match (&self.refused, unsaved, self.writable, self.seen.is_some()) {
            (Some(_), ..) => "Not shown",
            (None, true, ..) => "Not saved",
            (None, false, false, _) => "Read only",
            (None, false, true, true) => "Saved",
            (None, false, true, false) => "New",
        };
        let path = match &self.path {
            Some(path) => escape(&path.to_string_lossy()),
            None => "A new file, not saved anywhere yet".into(),
        };
        let trouble = self
            .trouble
            .as_ref()
            .map(|why| format!("<p class=\"trouble\">{}</p>", escape(why)))
            .unwrap_or_default();
        let body = match &self.refused {
            Some(why) => format!("<p id=\"refused\" class=\"refused\">{}</p>", escape(why)),
            None => format!(
                "<textarea id=\"text\" name=\"text\" class=\"{wrap}\" maxlength=\"{most}\" spellcheck=\"false\" autocomplete=\"off\" \
                 autocapitalize=\"off\" aria-label=\"{name}\" fx-autofocus{readonly}></textarea>",
                wrap = if facts.fields.get("wrap").is_empty() {
                    "straight"
                } else {
                    "wrapped"
                },
                most = file::MOST,
                name = escape(&self.name()),
                readonly = if self.writable { "" } else { " readonly" },
            ),
        };
        let lines = if text.is_empty() {
            0
        } else {
            text.matches('\n').count() + usize::from(!text.ends_with('\n'))
        };
        let counted = match &self.refused {
            Some(_) => String::new(),
            None => format!(
                "{lines} {} · {} · {}",
                if lines == 1 { "line" } else { "lines" },
                size(text.len()),
                self.endings.name()
            ),
        };
        // The first element is never seen. It is what the chords are hung on.
        format!(
            "<div hidden>\
             <button type=\"button\" fx-key=\"Ctrl+S\" fx-click=\"save\"></button>\
             <button type=\"button\" fx-key=\"Ctrl+Shift+S\" fx-click=\"save-as\"></button>\
             </div>\
             <header class=\"bar\">\
             <button type=\"button\" fx-click=\"save\"{cannot}>Save</button>\
             <button type=\"button\" fx-click=\"save-as\"{nothing}>Save as</button>\
             <span class=\"path\" title=\"{path}\"><bdi>{path}</bdi></span>\
             <label class=\"wrap\"><input type=\"checkbox\" name=\"wrap\" value=\"on\"> Wrap lines</label>\
             </header>\
             {trouble}{asked}{body}\
             <footer class=\"status\"><span class=\"state\" data-state=\"{state}\">{state}</span><span>{counted}</span></footer>",
            cannot = if self.refused.is_some() || !(self.writable || self.path.is_none()) {
                " disabled"
            } else {
                ""
            },
            nothing = if self.refused.is_some() {
                " disabled"
            } else {
                ""
            },
            asked = self.asked(),
        )
    }

    fn event(&mut self, name: &str, _: &Value, fields: &mut Fields) {
        // Whatever was wrong was wrong with the last thing asked.
        self.trouble = None;
        let asking = self.asking.take();
        match (name, asking) {
            ("save", _) => {
                self.save(fields);
            }
            ("save-as", _) if self.refused.is_none() => self.ask_where(fields, false),
            ("saved-as", Some(Asking::Path { closing })) => {
                let typed = fields.get("path").to_string();
                if !self.save_as(&typed, fields) {
                    // Still asked, with what was typed to put right.
                    self.asking = Some(Asking::Path { closing });
                } else if closing {
                    self.close();
                }
            }
            ("close-saved", Some(Asking::Closing)) => {
                if self.path.is_none() {
                    self.ask_where(fields, true);
                } else if self.save(fields) {
                    self.close();
                }
            }
            ("close-discarded", Some(Asking::Closing)) => self.close(),
            ("load", Some(Asking::Changed)) => self.load(fields),
            // Kept, and saving it writes over what something else did.
            ("keep", Some(Asking::Changed)) => {}
            ("cancel", _) => {}
            // Anything else leaves what was being asked as it was.
            (_, asking) => self.asking = asking,
        }
    }

    fn closing(&mut self, fields: &mut Fields) -> bool {
        if !self.unsaved(fields) {
            return true;
        }
        self.asking = Some(Asking::Closing);
        false
    }
}

fn main() {
    let mut app = match App::connect() {
        Ok(app) => app,
        Err(e) => {
            eprintln!("gxwi-text-editor: no desktop to open on: {e}");
            std::process::exit(1);
        }
    };
    app.stylesheet(
        "/gxwi-text-editor.css",
        include_str!("gxwi-text-editor.css"),
    );
    // As the person would have meant it, from wherever this was started.
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .map(|path| std::path::absolute(&path).unwrap_or(path));
    let (editor, text) = Editor::on(path);
    let window = app.live(&editor.name(), editor);
    let closer = window.closer();
    window.update(|editor, fields| {
        fields.set("text", &text);
        editor.window = Some(closer);
    });
    let looked_at = window.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(LOOKS);
            // Shown again only when there is something to show.
            if looked_at.look(|editor, _, _| editor.differs()) {
                looked_at.update(|editor, fields| editor.look(fields));
            }
        }
    });
    if let Err(e) = app.run() {
        eprintln!("gxwi-text-editor: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
