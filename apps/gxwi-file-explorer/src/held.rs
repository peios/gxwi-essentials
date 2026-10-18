//! What has been cut or copied, waiting to be pasted.
//!
//! It is the session's and not a window's: every window of File Explorer is a
//! process of its own, and what is copied in one is pasted in another. So
//! it is kept in a file in the session's runtime directory, which GXWI
//! makes for each person, empties as a session starts, and lets nobody
//! else into. The file holds two lines: `cut` or `copy`, and the path.
//!
//! Where there is no such directory, as when File Explorer is run by hand, it is
//! kept by the window and goes no further.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// The name of the file, in the session's runtime directory.
// Keep the historical session clipboard name so old and renamed windows interoperate.
const NAME: &str = "gexora.held";

#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    pub path: PathBuf,
    /// Whether it is to be moved: pasted once, and gone from where it was.
    pub cut: bool,
}

pub struct Holding {
    file: Option<PathBuf>,
    here: Option<Held>,
}

impl Holding {
    /// In the session's runtime directory, if this is a session.
    pub fn session() -> Holding {
        let directory = std::env::var_os("GXWI_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|directory| directory.is_dir());
        Holding {
            file: directory.map(|directory| directory.join(NAME)),
            here: None,
        }
    }

    /// In the file at `file`, or by this value alone.
    #[cfg(test)]
    pub fn at(file: Option<&Path>) -> Holding {
        Holding {
            file: file.map(Path::to_path_buf),
            here: None,
        }
    }

    /// What is held, if anything is.
    pub fn get(&self) -> Option<Held> {
        let Some(file) = &self.file else {
            return self.here.clone();
        };
        let said = std::fs::read_to_string(file).ok()?;
        let (how, path) = said.split_once('\n')?;
        let path = path.strip_suffix('\n').unwrap_or(path);
        let cut = match how {
            "cut" => true,
            "copy" => false,
            _ => return None,
        };
        Some(Held {
            path: PathBuf::from(path),
            cut,
        })
        .filter(|held| held.path.is_absolute())
    }

    /// Holds `held` in place of whatever was, or nothing.
    pub fn set(&mut self, held: Option<Held>) -> std::io::Result<()> {
        let Some(file) = &self.file else {
            self.here = held;
            return Ok(());
        };
        match held {
            Some(held) => {
                let path = held.path.to_str().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "its name is not UTF-8")
                })?;
                if path.contains('\n') {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "its name has a line break in it",
                    ));
                }
                let (mut output, mut temporary) = Temporary::beside(file)?;
                writeln!(output, "{}\n{path}", if held.cut { "cut" } else { "copy" })?;
                std::fs::rename(
                    temporary.path.as_ref().expect("pending temporary file"),
                    file,
                )?;
                // The name now belongs to the clipboard; never remove a later
                // file another process might create at the temporary name.
                temporary.path = None;
                Ok(())
            }
            None => match std::fs::remove_file(file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
        }
    }
}

/// Owns only the temporary directory entry exclusively created for this write.
struct Temporary {
    path: Option<PathBuf>,
}

impl Temporary {
    fn beside(file: &Path) -> io::Result<(File, Self)> {
        let mut attempt = 0u64;
        loop {
            let suffix = if attempt == 0 {
                format!("{}.new", std::process::id())
            } else {
                format!("{}.{attempt}.new", std::process::id())
            };
            let path = file.with_extension(suffix);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(output) => return Ok((output, Self { path: Some(path) })),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    attempt = attempt.checked_add(1).ok_or(error)?;
                }
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            // Cleanup is best effort after a failed write; its original error
            // remains the useful diagnosis and no existing entry was replaced.
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_one_window_holds_another_finds() {
        let dir =
            std::env::temp_dir().join(format!("gxwi-file-explorer-held-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(NAME);
        let (mut one, other) = (Holding::at(Some(&file)), Holding::at(Some(&file)));
        assert_eq!(other.get(), None);
        let held = Held {
            path: "/home/alice/a file.txt".into(),
            cut: true,
        };
        one.set(Some(held.clone())).unwrap();
        assert_eq!(other.get(), Some(held));
        one.set(Some(Held {
            path: "/home/alice/other".into(),
            cut: false,
        }))
        .unwrap();
        assert_eq!(
            other.get(),
            Some(Held {
                path: "/home/alice/other".into(),
                cut: false
            })
        );
        assert!(
            one.set(Some(Held {
                path: "/home/alice/two\nlines".into(),
                cut: false
            }))
            .is_err()
        );
        assert_eq!(other.get().unwrap().path, Path::new("/home/alice/other"));
        one.set(None).unwrap();
        one.set(None).unwrap();
        assert_eq!(other.get(), None);
        // What is not what this writes holds nothing.
        for not_held in [
            "",
            "copy",
            "moved\n/somewhere\n",
            "copy\nnot/from/the/root\n",
        ] {
            std::fs::write(&file, not_held).unwrap();
            assert_eq!(other.get(), None, "{not_held:?}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn with_nowhere_to_keep_it_a_window_keeps_it_itself() {
        let mut one = Holding::at(None);
        let held = Held {
            path: "/a".into(),
            cut: false,
        };
        one.set(Some(held.clone())).unwrap();
        assert_eq!(one.get(), Some(held));
        assert_eq!(Holding::at(None).get(), None);
    }
    #[test]
    fn clipboard_writes_preserve_colliding_files_and_symlinks() {
        let dir =
            std::env::temp_dir().join(format!("gxwi-clipboard-collision-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join(NAME);
        let victim = dir.join("victim");
        std::fs::write(&victim, "untouched").unwrap();
        let collision = file.with_extension(format!("{}.new", std::process::id()));
        std::os::unix::fs::symlink(&victim, &collision).unwrap();
        let second = file.with_extension(format!("{}.1.new", std::process::id()));
        std::fs::write(&second, "also untouched").unwrap();
        let mut holding = Holding::at(Some(&file));
        let held = Held {
            path: "/tmp/chosen".into(),
            cut: false,
        };
        holding.set(Some(held.clone())).unwrap();
        assert_eq!(holding.get(), Some(held));
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "untouched");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "also untouched");
        assert!(collision.is_symlink());
        assert!(!file.is_symlink());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unrepresentable_paths_preserve_the_existing_clipboard() {
        use std::os::unix::ffi::OsStringExt;
        let dir =
            std::env::temp_dir().join(format!("gxwi-clipboard-encoding-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join(NAME);
        let mut holding = Holding::at(Some(&file));
        let original = Held {
            path: "/tmp/original".into(),
            cut: true,
        };
        holding.set(Some(original.clone())).unwrap();
        let path = std::ffi::OsString::from_vec(b"/tmp/not-utf8-\xff".to_vec()).into();
        assert_eq!(
            holding
                .set(Some(Held { path, cut: false }))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(holding.get(), Some(original));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_clipboard_rename_removes_only_its_temporary_file() {
        let dir =
            std::env::temp_dir().join(format!("gxwi-clipboard-cleanup-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join(NAME);
        std::fs::create_dir(&file).unwrap();
        let mut holding = Holding::at(Some(&file));
        assert!(
            holding
                .set(Some(Held {
                    path: "/tmp/chosen".into(),
                    cut: false
                }))
                .is_err()
        );
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        assert!(file.is_dir());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
