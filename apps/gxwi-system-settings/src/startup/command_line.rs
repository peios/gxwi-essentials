//! Atomic command-line replacement through an exclusively owned sibling file.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_STAGING: AtomicU64 = AtomicU64::new(0);
const MAX_STAGING_ATTEMPTS: usize = 128;

struct Staging {
    path: PathBuf,
    file: File,
    owned: bool,
}

impl Staging {
    fn create(path: PathBuf) -> io::Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok(Self {
            path,
            file,
            owned: true,
        })
    }

    fn beside(destination: &Path) -> io::Result<Self> {
        let filename = destination.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "the command-line path has no filename",
            )
        })?;
        for _ in 0..MAX_STAGING_ATTEMPTS {
            let sequence = NEXT_STAGING.fetch_add(1, Ordering::Relaxed);
            let mut name = filename.to_os_string();
            name.push(format!(".{}.{sequence}.new", std::process::id()));
            match Self::create(destination.with_file_name(name)) {
                Ok(staging) => return Ok(staging),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "no unused command-line staging filename",
        ))
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if self.owned {
            // Best-effort cleanup must not hide the original write/rename error.
            // This path was successfully created by this operation alone.
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Preserve the existing atomic rename and creation permissions, while refusing
/// to overwrite or follow any pre-existing staging path.
pub(super) fn write(destination: &Path, contents: &[u8]) -> io::Result<()> {
    let mut staging = Staging::beside(destination)?;
    staging.file.write_all(contents)?;
    fs::rename(&staging.path, destination)?;
    staging.owned = false;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            loop {
                let sequence = NEXT_STAGING.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir()
                    .join(format!("gxwi-cmdline-{}-{sequence}", std::process::id()));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("could not create test directory: {error}"),
                }
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn existing_staging_file_and_symlink_targets_are_untouched() {
        let scratch = Scratch::new();
        let destination = scratch.0.join("cmdline");
        let old_staging = scratch.0.join("cmdline.new");
        let other = scratch.0.join("other");
        fs::write(&old_staging, "keep the old file").unwrap();
        write(&destination, b"new command line\n").unwrap();
        assert_eq!(
            fs::read_to_string(&old_staging).unwrap(),
            "keep the old file"
        );
        fs::remove_file(&old_staging).unwrap();
        fs::write(&other, "keep the target").unwrap();
        symlink(&other, &old_staging).unwrap();
        write(&destination, b"second command line\n").unwrap();
        assert_eq!(fs::read_to_string(&other).unwrap(), "keep the target");
        assert!(fs::symlink_metadata(&old_staging).unwrap().is_symlink());
        assert!(!fs::symlink_metadata(&destination).unwrap().is_symlink());
        assert_eq!(fs::read(&destination).unwrap(), b"second command line\n");
    }

    #[test]
    fn exclusive_creation_refuses_collisions_without_cleaning_the_other_file() {
        let scratch = Scratch::new();
        let occupied = scratch.0.join("occupied");
        fs::write(&occupied, "keep me").unwrap();
        assert_eq!(
            Staging::create(occupied.clone()).err().unwrap().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read_to_string(&occupied).unwrap(), "keep me");
        let link = scratch.0.join("link");
        symlink(&occupied, &link).unwrap();
        assert_eq!(
            Staging::create(link).err().unwrap().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read_to_string(&occupied).unwrap(), "keep me");
    }

    #[test]
    fn rename_failure_cleans_only_the_owned_staging_file() {
        let scratch = Scratch::new();
        let destination = scratch.0.join("cmdline");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep"), "keep me").unwrap();
        assert!(write(&destination, b"new\n").is_err());
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(destination.join("keep")).unwrap(),
            "keep me"
        );
    }

    #[test]
    fn concurrent_writers_each_publish_a_complete_command_line() {
        let scratch = Scratch::new();
        let destination = scratch.0.join("cmdline");
        let writers: Vec<_> = (0..8)
            .map(|index| {
                let destination = destination.clone();
                std::thread::spawn(move || {
                    write(&destination, format!("writer-{index}\n").as_bytes()).unwrap()
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let final_text = fs::read_to_string(destination).unwrap();
        assert!((0..8).any(|index| final_text == format!("writer-{index}\n")));
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
    }
}
