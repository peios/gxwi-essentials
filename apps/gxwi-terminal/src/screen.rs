//! The terminal as it stands: what is on it, what has gone off the top of
//! it, and how it has been left.
//!
//! The pages draw the terminal, each with an emulator of its own, and none
//! of them is where it is kept. Everything printed is followed here as well,
//! so that a page that comes later, on whatever device, can be brought to
//! where the others are: it is sent what would print the terminal as it
//! stands, which is no longer what was printed to get it there.

use std::fmt::Write;

use vt100::{Color, Parser};

use crate::pty::Size;

/// How many lines that have gone off the top are kept.
const KEPT: usize = 5000;

/// What was said in what was printed that is about the terminal and not on
/// it.
#[derive(Default)]
struct Heard {
    /// What the terminal was last called, until it is taken.
    title: Option<String>,
}

impl vt100::Callbacks for Heard {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        let title: String = String::from_utf8_lossy(title)
            .chars()
            .filter(|c| !c.is_control())
            .collect();
        self.title = Some(title);
    }
}

pub struct Screen(Parser<Heard>);

impl Screen {
    pub fn new(size: Size) -> Screen {
        Screen(Parser::new_with_callbacks(
            size.rows,
            size.cols,
            KEPT,
            Heard::default(),
        ))
    }

    /// What the terminal has been called since this was last asked, if it
    /// has been called anything: a shell says where it is, and a program
    /// what it is doing.
    pub fn called(&mut self) -> Option<String> {
        self.0.callbacks_mut().title.take()
    }

    pub fn size(&self) -> Size {
        let (rows, cols) = self.0.screen().size();
        Size { rows, cols }
    }

    pub fn resize(&mut self, size: Size) {
        self.0.screen_mut().set_size(size.rows, size.cols);
    }

    /// Follows what was printed.
    pub fn print(&mut self, printed: &[u8]) {
        self.0.process(printed);
    }

    /// What prints the terminal as it stands, on one that is the same size
    /// and has nothing on it: the lines that have gone off the top, then
    /// the screen, with the cursor where it is and the modes as they were
    /// left. While a program has the whole screen that is all there is to
    /// show: what was there before it cannot be, until it gives the screen
    /// up.
    pub fn as_it_stands(&mut self) -> Vec<u8> {
        let Size { rows, cols } = self.size();
        let mut out = String::new();
        let screen = self.0.screen_mut();
        // Asking for more than there is finds out how much there is.
        screen.set_scrollback(usize::MAX);
        let gone = screen.scrollback();
        // Looked at a screenful at a time, from the oldest.
        let mut before = gone;
        while before > 0 {
            screen.set_scrollback(before);
            let here = before.min(usize::from(rows));
            for row in 0..here as u16 {
                // The last of them ends where the screen begins, even if
                // it ran on into the screen's first.
                let last = before == here && usize::from(row) == here - 1;
                line(screen, row, cols, !last, &mut out);
            }
            before -= here;
        }
        screen.set_scrollback(0);
        if gone > 0 {
            // Off the top of whatever is shown this, as they went off here.
            out.push_str("\x1b[m");
            out.push_str(&"\r\n".repeat(usize::from(rows) - 1));
        }
        let mut out = out.into_bytes();
        if screen.alternate_screen() {
            out.extend(b"\x1b[?1049h");
        }
        out.extend(screen.state_formatted());
        out
    }

    /// Whether a program has the whole screen, which is a screen of its own
    /// that it gives up when it is done.
    pub fn taken_whole(&self) -> bool {
        self.0.screen().alternate_screen()
    }
}

/// One line as it would be printed, and on to the next unless it ran on
/// into it and `running_on` lets it.
fn line(screen: &vt100::Screen, row: u16, cols: u16, running_on: bool, out: &mut String) {
    let drawn = |col| {
        screen.cell(row, col).is_some_and(|cell| {
            cell.has_contents() || cell.bgcolor() != Color::Default || cell.inverse()
        })
    };
    let wrapped = running_on && screen.row_wrapped(row);
    let end = if wrapped {
        cols
    } else {
        (0..cols)
            .rev()
            .find(|&col| drawn(col))
            .map_or(0, |last| last + 1)
    };
    let mut looks = String::new();
    for col in 0..end {
        let Some(cell) = screen.cell(row, col) else {
            break;
        };
        if cell.is_wide_continuation() {
            continue;
        }
        let these = looks_of(cell);
        if these != looks {
            out.push_str(&these);
            looks = these;
        }
        out.push_str(if cell.has_contents() {
            cell.contents()
        } else {
            " "
        });
    }
    if !looks.is_empty() && looks != "\x1b[m" {
        out.push_str("\x1b[m");
    }
    if !wrapped {
        out.push_str("\r\n");
    }
}

/// What sets how a cell looks, from however the one before it looked.
fn looks_of(cell: &vt100::Cell) -> String {
    let mut looks = String::from("\x1b[");
    let flags = [
        (cell.bold(), "1"),
        (cell.dim(), "2"),
        (cell.italic(), "3"),
        (cell.underline(), "4"),
        (cell.inverse(), "7"),
    ];
    for (_, code) in flags.iter().filter(|(set, _)| *set) {
        let _ = write!(looks, ";{code}");
    }
    for (color, plain, bright, other) in
        [(cell.fgcolor(), 30, 90, 38), (cell.bgcolor(), 40, 100, 48)]
    {
        let _ = match color {
            Color::Default => Ok(()),
            Color::Idx(n @ 0..8) => write!(looks, ";{}", plain + u16::from(n)),
            Color::Idx(n @ 8..16) => write!(looks, ";{}", bright + u16::from(n) - 8),
            Color::Idx(n) => write!(looks, ";{other};5;{n}"),
            Color::Rgb(r, g, b) => write!(looks, ";{other};2;{r};{g};{b}"),
        };
    }
    looks.push('m');
    // Nothing set is said as the shortest way of setting nothing.
    if looks == "\x1b[m" {
        looks
    } else {
        looks.replacen("\x1b[;", "\x1b[0;", 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A terminal of the same size with nothing on it, shown `screen` as it
    /// stands.
    fn shown(screen: &mut Screen) -> Screen {
        let mut later = Screen::new(screen.size());
        later.print(&screen.as_it_stands());
        later
    }

    /// Every line there is, those gone off the top first.
    fn lines(screen: &mut Screen) -> Vec<String> {
        let Size { rows, cols } = screen.size();
        let inner = screen.0.screen_mut();
        inner.set_scrollback(usize::MAX);
        let gone = inner.scrollback();
        let mut all = Vec::new();
        let mut before = gone;
        while before > 0 {
            inner.set_scrollback(before);
            let here = before.min(usize::from(rows));
            all.extend(inner.rows(0, cols).take(here));
            before -= here;
        }
        inner.set_scrollback(0);
        all.extend(inner.rows(0, cols));
        all
    }

    #[test]
    fn a_page_that_comes_later_is_shown_the_screen_with_the_cursor_where_it_is() {
        let mut screen = Screen::new(Size { rows: 5, cols: 20 });
        screen.print(b"one\r\n\x1b[1;31mtwo\x1b[m\r\nthree and \x1b[44mblue\x1b[m\r\n$ ");
        let mut later = shown(&mut screen);
        assert_eq!(later.0.screen().contents(), screen.0.screen().contents());
        assert_eq!(later.0.screen().cursor_position(), (3, 2));
        let cell = later.0.screen().cell(1, 0).unwrap();
        assert!(cell.bold() && cell.fgcolor() == Color::Idx(1));
        assert_eq!(
            later.0.screen().cell(2, 10).unwrap().bgcolor(),
            Color::Idx(4)
        );
        assert_eq!(lines(&mut later), lines(&mut screen));
    }

    #[test]
    fn it_is_shown_what_has_gone_off_the_top_as_well() {
        let mut screen = Screen::new(Size { rows: 4, cols: 12 });
        for n in 1..=30 {
            screen.print(format!("\x1b[3{}mline {n}\x1b[m\r\n", n % 8).as_bytes());
        }
        // One that ran on into the next, and one with a wide character.
        screen.print("abcdefghijklmnopqrstuvwxyz\r\n日本語 text\r\n$ ".as_bytes());
        let before = lines(&mut screen);
        assert!(before.len() > 30, "{before:?}");
        assert_eq!(before[0], "line 1");
        let mut later = shown(&mut screen);
        assert_eq!(lines(&mut later), before);
        assert_eq!(
            later.0.screen().cursor_position(),
            screen.0.screen().cursor_position()
        );
        // The colours went with them.
        later.0.screen_mut().set_scrollback(usize::MAX);
        assert_eq!(
            later.0.screen().cell(0, 0).unwrap().fgcolor(),
            Color::Idx(1)
        );
    }

    #[test]
    fn a_program_with_the_whole_screen_is_shown_as_having_it() {
        let mut screen = Screen::new(Size { rows: 4, cols: 12 });
        screen.print(b"before\r\n\x1b[?1049h\x1b[2J\x1b[Hwhole\x1b[?25l\x1b[?2004h");
        let later = shown(&mut screen);
        assert!(later.0.screen().alternate_screen());
        assert!(later.0.screen().hide_cursor());
        assert!(later.0.screen().bracketed_paste());
        assert_eq!(later.0.screen().contents(), "whole");
    }

    #[test]
    fn what_the_terminal_is_called_is_heard_once() {
        let mut screen = Screen::new(Size::USUAL);
        assert_eq!(screen.called(), None);
        screen.print(b"\x1b]2;first\x07\x1b]0;alice: ~/work\x1b\\$ ");
        assert_eq!(screen.called().as_deref(), Some("alice: ~/work"));
        assert_eq!(screen.called(), None);
        assert_eq!(screen.0.screen().contents(), "$ ");
    }

    #[test]
    fn a_terminal_is_as_big_as_it_was_last_made() {
        let mut screen = Screen::new(Size::USUAL);
        screen.resize(Size { rows: 10, cols: 40 });
        assert_eq!(screen.size(), Size { rows: 10, cols: 40 });
    }
}
