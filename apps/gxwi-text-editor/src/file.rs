//! A file of text, read and written.
//!
//! Text here is UTF-8 with no NUL in it, and anything else is not text: it
//! is refused rather than shown as something it is not and saved as damage.
//! In the editor a line ends with a line feed, which is what a field of a
//! page holds whatever was put in it. A file whose lines all end with a
//! carriage return and a line feed is written back that way.
//!
//! A file is written where it is and not replaced by another of its name:
//! what it is, whose it is and who may touch it are the file's, and a new
//! file in its place would have none of them.

use std::fmt;
use std::io::{self, Read as _, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::SystemTime;

/// The most text the editor holds, in bytes. A field of a live surface
/// holds so many bytes and a page counts what is typed another way, in
/// which one may be three bytes: a third of a field is what always fits.
pub const MOST: usize = libgxwi::MAX_FIELD_BYTES / 3;

/// How the lines of a file end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Endings {
    /// With a line feed, or there is one line at most.
    Feed,
    /// With a carriage return and a line feed, every one.
    ReturnFeed,
    /// Some one way and some the other, which is written back as `Feed`.
    Mixed,
}

impl Endings {
    fn of(text: &str) -> Endings {
        let feeds = text.matches('\n').count();
        match text.matches("\r\n").count() {
            0 => Endings::Feed,
            both if both == feeds => Endings::ReturnFeed,
            _ => Endings::Mixed,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Endings::Feed => "LF",
            Endings::ReturnFeed => "CRLF",
            Endings::Mixed => "LF (the file had both)",
        }
    }
}

/// A file as it was when it was last looked at: enough to tell that
/// something else has changed it since.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    modified: Option<SystemTime>,
    len: u64,
}

/// A file as read.
#[derive(Debug)]
pub struct Read {
    /// What it holds, its lines ending with a line feed.
    pub text: String,
    pub endings: Endings,
    pub seen: Seen,
}

/// Why a file is not shown.
#[derive(Debug, PartialEq)]
pub enum Refused {
    TooLarge(u64),
    NotText,
    NotAFile,
    Denied,
    Other(String),
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refused::TooLarge(len) => write!(
                f,
                "This file is too large to edit here: it is {len} bytes, and the most is {MOST}."
            ),
            Refused::NotText => write!(
                f,
                "This file is not text, so it is not shown: saving it from here would damage it."
            ),
            Refused::NotAFile => write!(f, "This is not a file."),
            Refused::Denied => write!(f, "You are not allowed to read this file."),
            Refused::Other(why) => write!(f, "This file could not be read: {why}."),
        }
    }
}

/// The file at `path` as it is now, or `None` if there is nothing there.
pub fn seen(path: &Path) -> io::Result<Option<Seen>> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(Some(Seen {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        })),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Reads the file at `path`, if it is text the editor can hold.
pub fn read(path: &Path) -> Result<Read, Refused> {
    let refused = |e: io::Error| match e.kind() {
        io::ErrorKind::PermissionDenied => Refused::Denied,
        _ => Refused::Other(e.to_string()),
    };
    // Open without waiting for a FIFO writer, then inspect this descriptor:
    // checking the pathname first would allow replacement before the read.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(refused)?;
    let metadata = file.metadata().map_err(refused)?;
    if !metadata.is_file() {
        return Err(Refused::NotAFile);
    }
    if metadata.len() > MOST as u64 {
        return Err(Refused::TooLarge(metadata.len()));
    }
    let bytes = read_bounded(file).map_err(refused)?;
    // It may have grown since it was measured.
    if bytes.len() > MOST {
        return Err(Refused::TooLarge(bytes.len() as u64));
    }
    if bytes.contains(&0) {
        return Err(Refused::NotText);
    }
    let text = String::from_utf8(bytes).map_err(|_| Refused::NotText)?;
    let endings = Endings::of(&text);
    let seen = Seen {
        modified: metadata.modified().ok(),
        len: metadata.len(),
    };
    Ok(Read {
        text: text.replace("\r\n", "\n"),
        endings,
        seen,
    })
}

/// Read one byte beyond the accepted limit to detect growth without allocating
/// an unbounded buffer, even when a file reports a smaller initial length.
fn read_bounded(reader: impl io::Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(MOST as u64 + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Writes `text` to the file at `path`, its lines ending as `endings` says.
/// `new` is a file that must not be there yet, and nothing is written over
/// one that is.
pub fn write(path: &Path, text: &str, endings: Endings, new: bool) -> io::Result<Seen> {
    let text = match endings {
        Endings::ReturnFeed => text.replace('\n', "\r\n"),
        Endings::Feed | Endings::Mixed => text.to_string(),
    };
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if new {
        options.create_new(true)
    } else {
        options.create(true).truncate(true)
    };
    let mut file = options.open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    let metadata = file.metadata()?;
    Ok(Seen {
        modified: metadata.modified().ok(),
        len: metadata.len(),
    })
}

/// Whether this process may write the file at `path`, or make it if it is
/// not there: the system's word, asked the way a write would ask it.
pub fn may_write(path: &Path) -> bool {
    let allowed = |path: &Path| {
        let Ok(cpath) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else {
            return false;
        };
        // SAFETY: a NUL-terminated string, live for the call.
        unsafe { libc::access(cpath.as_ptr(), libc::W_OK) == 0 }
    };
    match std::fs::symlink_metadata(path) {
        Ok(_) => allowed(path),
        Err(_) => path.parent().is_some_and(allowed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gxwi-text-editor-file-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_file_is_written_back_with_the_line_endings_it_had() {
        let dir = dir("endings");
        let path = dir.join("dos.txt");
        std::fs::write(&path, "one\r\ntwo\r\n").unwrap();
        let read = read(&path).unwrap();
        assert_eq!(
            (read.text.as_str(), read.endings),
            ("one\ntwo\n", Endings::ReturnFeed)
        );
        write(&path, "one\ntwo\nthree\n", read.endings, false).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"one\r\ntwo\r\nthree\r\n");

        std::fs::write(&path, "one\ntwo").unwrap();
        assert_eq!(super::read(&path).unwrap().endings, Endings::Feed);
        std::fs::write(&path, "one\r\ntwo\nthree").unwrap();
        let mixed = super::read(&path).unwrap();
        assert_eq!(
            (mixed.text.as_str(), mixed.endings),
            ("one\ntwo\nthree", Endings::Mixed)
        );
        write(&path, &mixed.text, mixed.endings, false).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"one\ntwo\nthree");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn what_is_not_text_or_is_too_large_is_refused() {
        let dir = dir("refused");
        let path = dir.join("thing");
        std::fs::write(&path, b"text\0with a nul").unwrap();
        assert_eq!(read(&path).unwrap_err(), Refused::NotText);
        std::fs::write(&path, [0xff, 0xfe, b'a']).unwrap();
        assert_eq!(read(&path).unwrap_err(), Refused::NotText);
        std::fs::write(&path, vec![b'a'; MOST + 1]).unwrap();
        assert_eq!(read(&path).unwrap_err(), Refused::TooLarge(MOST as u64 + 1));
        std::fs::write(&path, vec![b'a'; MOST]).unwrap();
        assert_eq!(read(&path).unwrap().text.len(), MOST);
        assert_eq!(read(&dir).unwrap_err(), Refused::NotAFile);
        assert!(matches!(
            read(&dir.join("nothing")).unwrap_err(),
            Refused::Other(_)
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_new_file_is_never_written_over_one_that_is_there() {
        let dir = dir("new");
        let path = dir.join("kept.txt");
        std::fs::write(&path, "kept").unwrap();
        assert_eq!(
            write(&path, "lost", Endings::Feed, true)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "kept");
        let made = write(&dir.join("made.txt"), "made", Endings::Feed, true).unwrap();
        assert_eq!(seen(&dir.join("made.txt")).unwrap(), Some(made));
        assert_eq!(seen(&dir.join("nothing")).unwrap(), None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_file_written_where_it_is_is_still_the_same_file() {
        use std::os::unix::fs::MetadataExt;
        let dir = dir("same");
        let path = dir.join("same.txt");
        std::fs::write(&path, "before").unwrap();
        let was = std::fs::metadata(&path).unwrap().ino();
        write(&path, "after", Endings::Feed, false).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), was);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "after");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn a_growing_input_is_read_only_one_byte_beyond_the_limit() {
        let bytes = read_bounded(io::repeat(b'a')).unwrap();
        assert_eq!(bytes.len(), MOST + 1);
    }

    #[test]
    fn a_fifo_is_rejected_without_waiting_for_a_writer() {
        let directory = dir("fifo");
        let path = directory.join("pipe");
        let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: cpath is a live NUL-terminated filename and mode is valid.
        assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
        assert_eq!(read(&path).unwrap_err(), Refused::NotAFile);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
