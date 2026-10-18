//! A pseudoterminal with a program on the far side of it.
//!
//! The program is started in a session of its own with the terminal as its
//! controlling terminal, which is what makes Ctrl+C reach what is running in
//! it and lets a shell do job control. This process keeps the near side: what
//! is written there is typed, and what is read there was printed.

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// How big a terminal is, in characters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}

impl Size {
    /// What a terminal is until a page says what there is room for.
    pub const USUAL: Size = Size { rows: 24, cols: 80 };

    /// The size asked for, kept to what a terminal can be: a page is not
    /// trusted to ask for something sensible.
    pub fn kept(rows: u64, cols: u64) -> Size {
        Size {
            rows: rows.clamp(2, 500) as u16,
            cols: cols.clamp(2, 1000) as u16,
        }
    }
}

/// The near side of a pseudoterminal.
pub struct Pty(OwnedFd);

impl Pty {
    /// Opens a pseudoterminal of `size` and starts `program` on it, in the
    /// directory `at`, with `environment` over this process's own.
    pub fn start(
        program: &Path,
        arguments: &[String],
        at: &Path,
        environment: &[(&str, &str)],
        size: Size,
    ) -> io::Result<(Pty, Child)> {
        // SAFETY: each descriptor is checked before it is owned, and the
        // name is read into a buffer passed with its length.
        let (near, far) = unsafe {
            let fd = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let near = OwnedFd::from_raw_fd(fd);
            if libc::grantpt(fd) < 0 || libc::unlockpt(fd) < 0 {
                return Err(io::Error::last_os_error());
            }
            let mut name = [0 as libc::c_char; 128];
            let failed = libc::ptsname_r(fd, name.as_mut_ptr(), name.len());
            if failed != 0 {
                return Err(io::Error::from_raw_os_error(failed));
            }
            let fd = libc::open(
                name.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC,
            );
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            (near, OwnedFd::from_raw_fd(fd))
        };
        let pty = Pty(near);
        pty.resize(size)?;

        let mut command = Command::new(program);
        command.args(arguments);
        command.current_dir(at);
        command.envs(environment.iter().copied());
        command.stdin(Stdio::from(far.try_clone()?));
        command.stdout(Stdio::from(far.try_clone()?));
        command.stderr(Stdio::from(far));
        // SAFETY: between fork and exec, and both calls are ones that may be
        // made there. By then the far side is the program's input.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn()?;
        Ok((pty, child))
    }

    /// Tells the terminal how big it is, and whatever is running in it that
    /// it changed.
    pub fn resize(&self, size: Size) -> io::Result<()> {
        let size = libc::winsize {
            ws_row: size.rows,
            ws_col: size.cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: the descriptor is this value's own, and the size is whole.
        if unsafe { libc::ioctl(self.0.as_raw_fd(), libc::TIOCSWINSZ, &raw const size) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// The near side once more, as a file: one to read what is printed from,
    /// one to write what is typed to.
    pub fn file(&self) -> io::Result<File> {
        Ok(File::from(self.0.try_clone()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn a_size_is_kept_to_what_a_terminal_can_be() {
        assert_eq!(
            Size::kept(40, 120),
            Size {
                rows: 40,
                cols: 120
            }
        );
        assert_eq!(Size::kept(0, 0), Size { rows: 2, cols: 2 });
        assert_eq!(
            Size::kept(u64::MAX, u64::MAX),
            Size {
                rows: 500,
                cols: 1000
            }
        );
    }

    /// Everything printed until the program ends, which is an error from a
    /// terminal with nothing on the far side any more.
    fn printed(pty: &Pty) -> String {
        let mut from = pty.file().unwrap();
        let mut all = Vec::new();
        let mut some = [0u8; 4096];
        while let Ok(n) = from.read(&mut some) {
            if n == 0 {
                break;
            }
            all.extend_from_slice(&some[..n]);
        }
        String::from_utf8_lossy(&all).into_owned()
    }

    #[test]
    fn what_is_started_has_the_terminal_and_its_size_and_is_where_it_was_put() {
        let size = Size { rows: 31, cols: 97 };
        let script = "stty size; pwd; echo $TERM; tty >/dev/null && echo a terminal";
        let (pty, mut child) = Pty::start(
            Path::new("/bin/sh"),
            &[],
            Path::new("/"),
            &[("TERM", "tried"), ("SCRIPT", script)],
            size,
        )
        .unwrap();
        pty.file()
            .unwrap()
            .write_all(b"eval \"$SCRIPT\"; exit\n")
            .unwrap();
        let printed = printed(&pty);
        child.wait().unwrap();
        assert!(printed.contains("31 97"), "{printed}");
        assert!(printed.contains("\r\n/\r\n"), "{printed}");
        assert!(printed.contains("\r\ntried\r\n"), "{printed}");
        assert!(printed.contains("a terminal"), "{printed}");
    }
}
