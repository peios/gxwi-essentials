//! Filesystem operations shared by desktop applications.

use std::ffi::{CString, OsStr};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// An opaque UI identity for one native filename, distinct from its display label.
///
/// Valid UTF-8 names remain unchanged. A slash cannot appear in a filename,
/// so the encoded namespace cannot collide with any ordinary filename.
/// Callers must look up this identity in their listing, never use it as a path.
pub fn entry_id(name: &OsStr) -> String {
    if let Some(name) = name.to_str() {
        return name.to_owned();
    }
    use std::fmt::Write as _;
    let mut id = String::from("/bytes/");
    for byte in name.as_bytes() {
        write!(&mut id, "{byte:02x}").expect("writing into a String cannot fail");
    }
    id
}

/// Rename an entry atomically, refusing to replace any existing destination.
///
/// Symlinks are renamed as entries, including dangling links. Both paths may
/// be relative to the current directory. Cross-filesystem moves return EXDEV;
/// unsupported filesystems return their error without a racy fallback.
pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    let path = |path: &Path| {
        CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a path contains NUL"))
    };
    let from = path(from)?;
    let to = path(to)?;
    // SAFETY: both C strings are NUL terminated and live through the call.
    // AT_FDCWD needs no owned descriptor; renameat2 retains neither pointer.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::ffi::OsStringExt;
    use std::sync::{Arc, Barrier};

    use super::*;

    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("gxwi-fs-{name}-{}", std::process::id()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // Test cleanup must also run during an assertion unwind.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn existing_files_and_dangling_links_are_never_replaced() {
        let dir = Scratch::new("existing");
        let from = dir.0.join("from");
        let to = dir.0.join("to");
        std::fs::write(&from, "source").unwrap();
        std::fs::write(&to, "destination").unwrap();
        assert_eq!(
            rename_noreplace(&from, &to).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read_to_string(&from).unwrap(), "source");
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "destination");
        std::fs::remove_file(&to).unwrap();
        std::os::unix::fs::symlink("missing", &to).unwrap();
        assert_eq!(
            rename_noreplace(&from, &to).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read_link(&to).unwrap(), Path::new("missing"));
    }

    #[test]
    fn concurrent_renames_have_one_winner_without_overwriting_it() {
        let dir = Scratch::new("race");
        let to = dir.0.join("winner");
        let barrier = Arc::new(Barrier::new(3));
        let workers: Vec<_> = ["one", "two"]
            .into_iter()
            .map(|name| {
                let from = dir.0.join(name);
                std::fs::write(&from, name).unwrap();
                let to = to.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    (name, from.clone(), rename_noreplace(&from, &to))
                })
            })
            .collect();
        barrier.wait();
        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(
            results
                .iter()
                .filter(|(_, _, result)| result.is_ok())
                .count(),
            1
        );
        for (name, from, result) in results {
            match result {
                Ok(()) => {
                    assert!(!from.exists());
                    assert_eq!(std::fs::read_to_string(&to).unwrap(), name);
                }
                Err(error) => {
                    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
                    assert_eq!(std::fs::read_to_string(from).unwrap(), name);
                }
            }
        }
    }

    #[test]
    fn nul_paths_are_rejected_without_touching_files() {
        let path = std::ffi::OsString::from_vec(b"invalid\0path".to_vec());
        assert_eq!(
            rename_noreplace(Path::new(&path), Path::new("unused"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
}
