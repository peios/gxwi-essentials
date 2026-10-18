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
            // A path with a line break in it would read back as another.
            Some(held) if held.path.to_string_lossy().contains('\n') => {
                Err(std::io::Error::other("its name has a line break in it"))
            }
            Some(held) => {
                // Written whole under another name, so that a window
                // reading it never finds half of it.
                let new = file.with_extension(format!("{}.new", std::process::id()));
                std::fs::write(
                    &new,
                    format!(
                        "{}\n{}\n",
                        if held.cut { "cut" } else { "copy" },
                        held.path.to_string_lossy()
                    ),
                )?;
                std::fs::rename(&new, file)
            }
            None => match std::fs::remove_file(file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
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
}
