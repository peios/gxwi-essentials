//! One terminal, and the pages looking at it.
//!
//! The terminal belongs to this process and not to any page. Pages come and
//! go, on whatever devices. Each is shown the terminal as it stands and then
//! everything printed, and none of them is needed for what is running in it
//! to carry on.
//!
//! There is one terminal and so one size, whatever the windows showing it
//! are: it is as big as the page that last said what it had room for, and
//! every page draws it that big.

use std::fs::File;
use std::io::{Read, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use tokio::sync::broadcast;

use crate::pty::{Pty, Size};
use crate::screen::Screen;

/// How much is read of what is printed at once, and how many such pieces a
/// page may fall behind by before it is shown the terminal afresh instead.
const PIECE: usize = 16 * 1024;
const BEHIND: usize = 256;

/// What every page is told, in the order it happened.
#[derive(Clone)]
pub enum Said {
    Printed(Bytes),
    Size(Size),
    /// What was running has ended, and nothing more will be printed.
    Ended,
    /// The terminal is to be shown afresh, to every page: some page has not
    /// got what is on it now.
    Afresh,
}

/// The terminal as it stands, and what has become of it.
struct Stands {
    screen: Screen,
    /// Whether what was running has ended.
    ended: bool,
    /// Whether a page came while a program had the whole screen. Such a
    /// page was not shown what was there before the program took it, so it
    /// cannot put that back when the program gives the screen up.
    came_to_the_whole: bool,
}

pub struct Terminal {
    /// Held while anything is printed or the size changes, and while a page
    /// is shown the terminal as it stands, so that a page misses nothing
    /// between that and what it is told next.
    stands: Mutex<Stands>,
    said: broadcast::Sender<Said>,
    typed: Mutex<mpsc::Sender<Vec<u8>>>,
    pty: Pty,
}

impl Terminal {
    /// The terminal on the near side of `pty`. Typing is begun here; what is
    /// printed is followed by whoever calls [`Terminal::follow`].
    pub fn new(pty: Pty, size: Size) -> std::io::Result<Arc<Terminal>> {
        let (typed, to_type) = mpsc::channel::<Vec<u8>>();
        let mut to = pty.file()?;
        // Writing waits while what is running is not reading, which is not
        // for a page's connection to wait on.
        std::thread::spawn(move || {
            for typed in to_type {
                if to.write_all(&typed).is_err() {
                    break;
                }
            }
        });
        Ok(Arc::new(Terminal {
            stands: Mutex::new(Stands { screen: Screen::new(size), ended: false, came_to_the_whole: false }),
            said: broadcast::channel(BEHIND).0,
            typed: Mutex::new(typed),
            pty,
        }))
    }

    /// Reads what is printed until there is no more, which is when nothing
    /// has the far side any longer.
    pub fn follow(&self) {
        let Ok(mut from) = self.pty.file() else { return self.ended() };
        self.follow_from(&mut from);
    }

    fn follow_from(&self, from: &mut File) {
        let mut piece = vec![0u8; PIECE];
        loop {
            match from.read(&mut piece) {
                Ok(0) => break,
                Ok(n) => {
                    let mut stands = self.stands.lock().unwrap_or_else(|e| e.into_inner());
                    let whole = stands.screen.taken_whole();
                    stands.screen.print(&piece[..n]);
                    // Nobody listening is not a failure.
                    let _ = self.said.send(Said::Printed(Bytes::copy_from_slice(&piece[..n])));
                    if whole && !stands.screen.taken_whole() && stands.came_to_the_whole {
                        stands.came_to_the_whole = false;
                        let _ = self.said.send(Said::Afresh);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        self.ended();
    }

    fn ended(&self) {
        let mut stands = self.stands.lock().unwrap_or_else(|e| e.into_inner());
        stands.ended = true;
        let _ = self.said.send(Said::Ended);
    }

    /// For a page that has come: how big the terminal is, what prints it as
    /// it stands, whether what was running has ended, and where what is said
    /// from now on is heard.
    pub fn join(&self) -> (Size, Vec<u8>, bool, broadcast::Receiver<Said>) {
        let mut stands = self.stands.lock().unwrap_or_else(|e| e.into_inner());
        stands.came_to_the_whole |= stands.screen.taken_whole();
        (stands.screen.size(), stands.screen.as_it_stands(), stands.ended, self.said.subscribe())
    }

    /// What a person typed, or pasted, or what a page's emulator answered
    /// for the terminal.
    pub fn type_in(&self, typed: Vec<u8>) {
        let _ = self.typed.lock().unwrap_or_else(|e| e.into_inner()).send(typed);
    }

    /// Makes the terminal the size a page has room for.
    pub fn resize(&self, size: Size) {
        let mut stands = self.stands.lock().unwrap_or_else(|e| e.into_inner());
        if stands.screen.size() == size || stands.ended {
            return;
        }
        if let Err(e) = self.pty.resize(size) {
            eprintln!("peiterm: making the terminal {} by {}: {e}", size.cols, size.rows);
            return;
        }
        stands.screen.resize(size);
        let _ = self.said.send(Said::Size(size));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn printed(said: &mut broadcast::Receiver<Said>) -> (String, Vec<Size>, bool) {
        let (mut text, mut sizes, mut ended) = (Vec::new(), Vec::new(), false);
        while let Ok(said) = said.try_recv() {
            match said {
                Said::Printed(bytes) => text.extend_from_slice(&bytes),
                Said::Size(size) => sizes.push(size),
                Said::Ended => ended = true,
                Said::Afresh => {}
            }
        }
        (String::from_utf8_lossy(&text).into_owned(), sizes, ended)
    }

    #[test]
    fn what_is_typed_is_run_and_a_page_that_comes_after_is_shown_what_came_of_it() {
        let size = Size { rows: 10, cols: 40 };
        let (pty, mut child) = Pty::start(Path::new("/bin/sh"), &[], Path::new("/"), &[("PS1", "")], size).unwrap();
        let terminal = Terminal::new(pty, size).unwrap();
        let (at_first, _, ended, mut said) = terminal.join();
        assert_eq!(at_first, size);
        assert!(!ended);

        terminal.resize(Size { rows: 12, cols: 50 });
        terminal.type_in(b"stty size; echo done-$((6*7)); exit\n".to_vec());
        terminal.follow();
        child.wait().unwrap();

        let (text, sizes, ended) = printed(&mut said);
        assert!(text.contains("12 50"), "{text}");
        assert!(text.contains("done-42"), "{text}");
        assert_eq!(sizes, [Size { rows: 12, cols: 50 }]);
        assert!(ended);

        let (size, stands, ended, _) = terminal.join();
        assert_eq!(size, Size { rows: 12, cols: 50 });
        assert!(String::from_utf8_lossy(&stands).contains("done-42"));
        assert!(ended);
    }
}
