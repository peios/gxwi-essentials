//! peiterm: a terminal on a GXWI desktop.
//!
//! A window with the person's shell in it, on a pseudoterminal of this
//! process's. Anything after the program's name is run in place of the
//! shell, the first of it the program and the rest what it is given.
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
// `peiterm.svg`, installed as the base theme's `dev.peios.peiterm.svg`.
libgxwi::icon!(b"dev.peios.peiterm");

/// What the emulator in the page is, to the programs that ask.
const TERM: &str = "xterm-256color";

fn main() {
    let mut run: Vec<String> = std::env::args().skip(1).collect();
    if run.is_empty() {
        run.push(std::env::var("SHELL").ok().filter(|shell| shell.starts_with('/')).unwrap_or_else(|| "/bin/sh".into()));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).filter(|home| home.is_dir()).unwrap_or_else(|| "/".into());

    let mut app = App::connect().unwrap_or_else(|e| die(&format!("the display: {e}")));
    let environment = [("TERM", TERM), ("COLORTERM", "truecolor")];
    let (pty, mut child) = Pty::start(run[0].as_ref(), &run[1..], &home, &environment, Size::USUAL)
        .unwrap_or_else(|e| die(&format!("starting {}: {e}", run[0])));
    let terminal = Terminal::new(pty, Size::USUAL).unwrap_or_else(|e| die(&format!("the terminal: {e}")));

    app.routes(page::routes(terminal.clone()));
    let window = app.raw_page("Terminal", page::PATH);
    std::thread::spawn(move || {
        terminal.follow();
        let _ = child.wait();
        window.close();
    });
    app.run().unwrap_or_else(|e| die(&format!("the display: {e}")));
}

fn die(message: &str) -> ! {
    eprintln!("peiterm: {message}");
    std::process::exit(1);
}
