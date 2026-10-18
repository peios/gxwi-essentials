//! Copying and moving, which is the work that may take a while.
//!
//! Nothing here writes over anything: what is made is made new, and fails
//! if something has its name. A link is copied as the link it is, not as
//! what it leads to. What is neither a file, a directory nor a link is not
//! copied, and that is said.

use std::io;
use std::path::{Path, PathBuf};

/// Whether `name` is a name a file can have.
pub fn is_name(name: &str) -> bool {
    !(name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\0'))
}

/// Whether there is anything at `path`, a link that leads nowhere included.
pub fn taken(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// A path in `into` for a copy of something called `name`: by that name if
/// nothing has it, and otherwise by one that says it is a copy, before
/// whatever says what kind of file it is.
pub fn free(into: &Path, name: &str) -> PathBuf {
    if !taken(&into.join(name)) {
        return into.join(name);
    }
    let (stem, kind) = match name.rfind('.') {
        Some(at) if at > 0 => name.split_at(at),
        _ => (name, ""),
    };
    (1u32..)
        .map(|n| {
            if n == 1 {
                format!("{stem} (copy){kind}")
            } else {
                format!("{stem} (copy {n}){kind}")
            }
        })
        .map(|name| into.join(name))
        .find(|path| !taken(path))
        .unwrap_or_else(|| into.join(name))
}

/// Copies what is at `from` to `to`, where there is nothing: a directory
/// with everything in it.
pub fn copy(from: &Path, to: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(from)?;
    if metadata.is_symlink() {
        std::os::unix::fs::symlink(std::fs::read_link(from)?, to)
    } else if metadata.is_dir() {
        std::fs::create_dir(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        // Last, so that one nobody may write in can still be filled.
        std::fs::set_permissions(to, metadata.permissions())
    } else if metadata.is_file() {
        if taken(to) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "there is already something there",
            ));
        }
        std::fs::copy(from, to).map(|_| ())
    } else {
        Err(io::Error::other(format!(
            "{} is not a file, a directory or a link",
            from.display()
        )))
    }
}

/// Moves what is at `from` to `to`, where there is nothing. Between two
/// volumes that is a copy, and then what was copied deleted.
pub fn move_to(from: &Path, to: &Path) -> io::Result<()> {
    if taken(to) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "there is already something there",
        ));
    }
    match std::fs::rename(from, to) {
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            copy(from, to)?;
            delete(from)
        }
        moved => moved,
    }
}

/// Deletes what is at `path`: a directory with everything in it, and a
/// link as the link it is.
pub fn delete(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path)? {
        metadata if metadata.is_dir() => std::fs::remove_dir_all(path),
        _ => std::fs::remove_file(path),
    }
}

/// Whether the file at `path` reads as text, as far as its beginning says:
/// UTF-8 with no NUL in it. One with nothing in it does.
pub fn reads_as_text(path: &Path) -> bool {
    use std::io::Read;
    let mut beginning = vec![0u8; 4096];
    let Ok(read) = std::fs::File::open(path).and_then(|mut file| file.read(&mut beginning)) else {
        return false;
    };
    let beginning = &beginning[..read];
    if beginning.contains(&0) {
        return false;
    }
    match std::str::from_utf8(beginning) {
        Ok(_) => true,
        // A character cut short by where the reading stopped is no fault.
        Err(e) => e.error_len().is_none() && read - e.valid_up_to() < 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gxwi-file-explorer-work-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_copy_is_given_a_name_nothing_has() {
        let dir = dir("free");
        let name = |name: &str| {
            free(&dir, name)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        };
        assert_eq!(name("notes.txt"), "notes.txt");
        std::fs::write(dir.join("notes.txt"), "").unwrap();
        assert_eq!(name("notes.txt"), "notes (copy).txt");
        std::fs::write(dir.join("notes (copy).txt"), "").unwrap();
        assert_eq!(name("notes.txt"), "notes (copy 2).txt");
        // What begins with a dot has no kind after it, and neither has a folder.
        std::fs::write(dir.join(".profile"), "").unwrap();
        assert_eq!(name(".profile"), ".profile (copy)");
        std::fs::create_dir(dir.join("Music")).unwrap();
        assert_eq!(name("Music"), "Music (copy)");
        std::fs::write(dir.join("archive.tar.gz"), "").unwrap();
        assert_eq!(name("archive.tar.gz"), "archive.tar (copy).gz");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_directory_is_copied_with_everything_in_it_and_links_as_links() {
        let dir = dir("copy");
        std::fs::create_dir_all(dir.join("from/inner/deeper")).unwrap();
        std::fs::write(dir.join("from/one.txt"), "one").unwrap();
        std::fs::write(dir.join("from/inner/deeper/two.txt"), "two").unwrap();
        std::os::unix::fs::symlink("one.txt", dir.join("from/link")).unwrap();
        std::os::unix::fs::symlink("/nowhere/at/all", dir.join("from/broken")).unwrap();
        copy(&dir.join("from"), &dir.join("to")).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("to/one.txt")).unwrap(),
            "one"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("to/inner/deeper/two.txt")).unwrap(),
            "two"
        );
        assert_eq!(
            std::fs::read_link(dir.join("to/link")).unwrap(),
            Path::new("one.txt")
        );
        assert_eq!(
            std::fs::read_link(dir.join("to/broken")).unwrap(),
            Path::new("/nowhere/at/all")
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("from/one.txt")).unwrap(),
            "one"
        );
        // Nothing is copied onto what is there.
        assert!(copy(&dir.join("from"), &dir.join("to")).is_err());
        assert_eq!(
            copy(&dir.join("from/one.txt"), &dir.join("to/one.txt"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn what_is_moved_is_gone_from_where_it_was_and_onto_nothing() {
        let dir = dir("move");
        std::fs::create_dir_all(dir.join("from/inner")).unwrap();
        std::fs::write(dir.join("from/inner/one.txt"), "one").unwrap();
        std::fs::write(dir.join("taken"), "kept").unwrap();
        assert_eq!(
            move_to(&dir.join("from"), &dir.join("taken"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read_to_string(dir.join("taken")).unwrap(), "kept");
        move_to(&dir.join("from"), &dir.join("to")).unwrap();
        assert!(!dir.join("from").exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("to/inner/one.txt")).unwrap(),
            "one"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn text_is_told_from_what_is_not_by_its_beginning() {
        let dir = dir("text");
        let reads = |bytes: &[u8]| {
            std::fs::write(dir.join("thing"), bytes).unwrap();
            reads_as_text(&dir.join("thing"))
        };
        assert!(reads(b"all: build\n\tcargo build\n"));
        assert!(reads(b""));
        assert!(reads("naïve café 日本語".as_bytes()));
        assert!(!reads(b"\x7fELF\x02\x01\x01\0\0\0"));
        assert!(!reads(&[0xff, 0xfe, b'a', b'b']));
        // Long enough that a character is cut where the reading stops.
        let long = "é".repeat(4000);
        assert!(reads(&long.as_bytes()[1..]) == false);
        assert!(reads(format!("a{long}").as_bytes()));
        assert!(!reads_as_text(&dir.join("nothing")));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
