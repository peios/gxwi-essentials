//! gxwi-terminal: a terminal on a GXWI desktop.
//!
//! A window with the person's shell in it, on a pseudoterminal of this
//! process's. Anything after the program's name is run in place of the
//! shell, the first of it the program and the rest what it is given; except
//! that a directory by itself is where the shell is opened, in place of the
//! person's home. The window is called what the shell calls the terminal,
//! if it calls it anything.
//!
//! The window is a page of the app's own and not a live surface, which a
//! terminal cannot be: what is printed is drawn by an emulator in the page,
//! as fast as it comes. What the page holds is still only a drawing. The
//! terminal is here (`terminal`, `screen`), so it is the same on every
//! device looking at it, and is all still there when a browser tab is
//! closed and opened again.
//!
//! What is started has the display in its environment as this did, so a
//! program run from the terminal can be an app on the desktop.
//!
//! The shell ending lets the window go, and the person closing the window
//! ends this process, which hangs the terminal up on whatever is in it.

use std::path::PathBuf;

use libgxwi::App;

mod page;
mod pty;
mod screen;
mod terminal;

use pty::{Pty, Size};
use terminal::Terminal;

// What this program looks like, to whatever lists it. The icon itself is
// `gxwi-terminal.svg`, installed as the base theme's `dev.peios.gxwi-terminal.svg`.
libgxwi::icon!(b"dev.peios.gxwi-terminal");

/// What the emulator in the page is, to the programs that ask.
const TERM: &str = "xterm-256color";

fn main() {
    let mut run: Vec<String> = std::env::args().skip(1).collect();
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_dir())
        .unwrap_or_else(|| "/".into());
    let home = match within(&run) {
        Some(directory) => {
            run.clear();
            directory
        }
        None => home,
    };
    if run.is_empty() {
        run.push(
            std::env::var("SHELL")
                .ok()
                .filter(|shell| shell.starts_with('/'))
                .unwrap_or_else(|| "/bin/sh".into()),
        );
    }

    let mut app = App::connect().unwrap_or_else(|e| die(&format!("the display: {e}")));
    let environment = [("TERM", TERM), ("COLORTERM", "truecolor")];
    let (pty, mut child) = Pty::start(run[0].as_ref(), &run[1..], &home, &environment, Size::USUAL)
        .unwrap_or_else(|e| die(&format!("starting {}: {e}", run[0])));
    let terminal =
        Terminal::new(pty, Size::USUAL).unwrap_or_else(|e| die(&format!("the terminal: {e}")));

    app.routes(page::routes(terminal.clone()));
    let window = app.raw_page("Terminal", page::PATH);
    std::thread::spawn(move || {
        // Called what the shell calls it, and Terminal when that is nothing.
        terminal.follow(|title| {
            window.retitle(if title.trim().is_empty() {
                "Terminal"
            } else {
                title
            })
        });
        let _ = child.wait();
        window.close();
    });
    app.run()
        .unwrap_or_else(|e| die(&format!("the display: {e}")));
}

/// The directory to open the shell in, when that is what was asked: one
/// argument, which is a directory. It is how a directory is opened with
/// the terminal (PGSS Applications), from a file explorer.
fn within(arguments: &[String]) -> Option<PathBuf> {
    match arguments {
        [only] => Some(PathBuf::from(only)).filter(|path| path.is_absolute() && path.is_dir()),
        _ => None,
    }
}

fn die(message: &str) -> ! {
    eprintln!("gxwi-terminal: {message}");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_argument_that_is_a_directory_is_where_the_shell_opens() {
        let said = |arguments: &[&str]| {
            within(&arguments.iter().map(|a| a.to_string()).collect::<Vec<_>>())
        };
        assert_eq!(said(&["/"]), Some(PathBuf::from("/")));
        assert_eq!(said(&[]), None);
        // A program, a directory that is not there, one not said from the
        // root, and a directory with something after it.
        assert_eq!(said(&["/bin/sh"]), None);
        assert_eq!(said(&["/no/such/place"]), None);
        assert_eq!(said(&["."]), None);
        assert_eq!(said(&["/", "-l"]), None);
    }
}
