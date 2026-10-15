//! One save file and previous-file backup in a host-selected user data directory.
use crate::{Context, Error, MAX_SAVE_BYTES, SavedGame};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// Older/corrupt slots can be backed up without loading them into memory.
pub const MAX_BACKUP_BYTES: u64 = 64 * 1024 * 1024;

pub struct Store {
    directory: PathBuf,
}

struct WriterLock(File);
impl Drop for WriterLock {
    fn drop(&mut self) {
        // Closing only this handle can leave a lock held by inherited/duplicated handles.
        let _ = self.0.unlock();
    }
}

impl Store {
    /// The adapter must resolve user data storage outside the Story package.
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
    pub fn path(&self) -> PathBuf {
        self.directory.join("save.json")
    }
    pub fn backup_path(&self) -> PathBuf {
        self.directory.join("save.bak")
    }

    pub fn load(&self, context: &Context) -> Result<SavedGame, Error> {
        SavedGame::from_bytes(&read(&self.path())?, context)
    }

    pub fn write(&self, game: &SavedGame) -> Result<(), Error> {
        let bytes = game.to_bytes()?;
        fs::create_dir_all(&self.directory).map_err(|source| io("create directory", source))?;
        let lock_path = self.directory.join("save.lock");
        reject_link(&lock_path)?;
        // Keep the lock inode: deleting it would let another writer lock a different file.
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| io("open writer lock", source))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Err(Error::Busy),
            Err(std::fs::TryLockError::Error(source)) => return Err(io("lock writer", source)),
        }
        let _guard = WriterLock(lock);
        reject_link(&self.path())?;
        reject_link(&self.backup_path())?;
        let candidate = temporary(&self.directory, &bytes)?;
        match File::open(self.path()) {
            Ok(previous) => {
                if previous
                    .metadata()
                    .map_err(|source| io("inspect previous slot", source))?
                    .len()
                    > MAX_BACKUP_BYTES
                {
                    return Err(Error::TooLarge {
                        resource: "previous slot backup; preserve or move the existing slot before saving",
                        limit: MAX_BACKUP_BYTES as usize,
                    });
                }
                let mut backup = NamedTempFile::new_in(&self.directory)
                    .map_err(|source| io("create backup", source))?;
                let copied = std::io::copy(&mut previous.take(MAX_BACKUP_BYTES + 1), &mut backup)
                    .map_err(|source| io("copy previous slot", source))?;
                if copied > MAX_BACKUP_BYTES {
                    return Err(Error::TooLarge {
                        resource: "previous slot backup",
                        limit: MAX_BACKUP_BYTES as usize,
                    });
                }
                backup
                    .as_file()
                    .sync_all()
                    .map_err(|source| io("flush backup", source))?;
                backup
                    .persist(self.backup_path())
                    .map_err(|error| io("replace backup", error.error))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(io("open previous slot", source)),
        }
        candidate
            .persist(self.path())
            .map_err(|error| io("replace main slot", error.error))?;
        Ok(())
    }
}

fn io(operation: &'static str, source: std::io::Error) -> Error {
    Error::Io { operation, source }
}

fn temporary(directory: &Path, bytes: &[u8]) -> Result<NamedTempFile, Error> {
    let mut temporary =
        NamedTempFile::new_in(directory).map_err(|source| io("create temporary", source))?;
    temporary
        .write_all(bytes)
        .map_err(|source| io("write temporary", source))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|source| io("flush temporary", source))?;
    Ok(temporary)
}

fn reject_link(path: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let link = metadata.file_type().is_symlink();
            #[cfg(windows)]
            let link = {
                use std::os::windows::fs::MetadataExt;
                link || metadata.file_attributes() & 0x400 != 0
            };
            if link {
                return Err(Error::Invalid(format!(
                    "save path '{}' is a link or reparse point",
                    path.display()
                )));
            }
            if !metadata.is_file() {
                return Err(Error::Invalid(format!(
                    "save path '{}' is not a regular file",
                    path.display()
                )));
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io("inspect path", source)),
    }
}

fn read(path: &Path) -> Result<Vec<u8>, Error> {
    reject_link(path)?;
    let file = File::open(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            Error::Missing
        } else {
            io("open slot", source)
        }
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_SAVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| io("read slot", source))?;
    if bytes.len() > MAX_SAVE_BYTES {
        return Err(Error::TooLarge {
            resource: "save",
            limit: MAX_SAVE_BYTES,
        });
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_guard_unlocks_while_a_duplicate_handle_is_still_open() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .unwrap();
        file.try_lock().unwrap();
        let duplicate = file.try_clone().unwrap();
        drop(WriterLock(file));
        let next = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        next.try_lock().unwrap();
        next.unlock().unwrap();
        drop(duplicate);
    }
}
