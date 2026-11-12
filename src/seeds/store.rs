//! On-disk seed storage: `<seed>.async.js` and `<seed>.raw.js` siblings in one
//! directory, and nothing else.
//!
//! A raw kernel is never overwritten. Saving a kernel that is already present
//! for its seed is reported as a duplicate; a kernel with the same seed token
//! but different bytes is kept beside the first under
//! `<seed>.<digest>.raw.js`, where `<digest>` is the leading hex of the
//! kernel's SHA-256, with its launcher at the matching `.async.js` name.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::seeds::error::{Error, Result};
use crate::seeds::script::sha256_hex;
use crate::seeds::seed::SeedToken;

/// Hex digits of the kernel digest used in a variant file name.
const SHORT_DIGEST_LEN: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreStatus {
    /// First kernel seen for this seed; written at `<seed>.raw.js`.
    Stored,
    /// These exact kernel bytes were already stored for this seed.
    Duplicate,
    /// Same seed token, different kernel bytes; written at a digest name.
    Variant,
}

impl StoreStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            StoreStatus::Stored => "stored",
            StoreStatus::Duplicate => "duplicate",
            StoreStatus::Variant => "variant",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreOutcome {
    pub status: StoreStatus,
    pub kernel_path: PathBuf,
    /// The launcher sibling of `kernel_path`, if one exists on disk.
    pub launcher_path: Option<PathBuf>,
    /// A duplicate kernel arrived with a launcher whose bytes differ from the
    /// stored sibling. The stored launcher is kept; the new one is not saved.
    pub launcher_differs: bool,
}

pub struct SeedStore {
    dir: PathBuf,
}

enum Slot {
    Free,
    Same,
    Taken,
}

impl SeedStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Save a kernel and, when known, the launcher that loaded it.
    pub fn save(
        &self,
        seed: &SeedToken,
        launcher: Option<&[u8]>,
        kernel: &[u8],
    ) -> Result<StoreOutcome> {
        fs::create_dir_all(&self.dir).map_err(|error| Error::io(&self.dir, error))?;
        let digest = sha256_hex(kernel);
        let stems = [
            seed.to_string(),
            format!("{seed}.{}", &digest[..SHORT_DIGEST_LEN]),
            format!("{seed}.{digest}"),
        ];
        for (index, stem) in stems.iter().enumerate() {
            let kernel_path = self.dir.join(format!("{stem}.raw.js"));
            let launcher_path = self.dir.join(format!("{stem}.async.js"));
            let kernel_slot = slot(&kernel_path, Some(kernel))?;
            let launcher_slot = slot(&launcher_path, launcher)?;
            let status = match (&kernel_slot, &launcher_slot) {
                (Slot::Taken, _) => continue,
                // An unpaired launcher with other bytes owns this name.
                (Slot::Free, Slot::Taken) => continue,
                (Slot::Same, _) => StoreStatus::Duplicate,
                (Slot::Free, _) if index == 0 => StoreStatus::Stored,
                (Slot::Free, _) => StoreStatus::Variant,
            };
            let wrote_launcher = match (launcher, &launcher_slot) {
                (Some(bytes), Slot::Free) => {
                    write_new(&launcher_path, bytes)?;
                    true
                }
                _ => false,
            };
            if matches!(kernel_slot, Slot::Free) {
                if let Err(error) = write_new(&kernel_path, kernel) {
                    if wrote_launcher {
                        let _ = fs::remove_file(&launcher_path);
                    }
                    return Err(error);
                }
            }
            return Ok(StoreOutcome {
                status,
                kernel_path,
                launcher_differs: matches!(launcher_slot, Slot::Taken),
                launcher_path: launcher_path.exists().then_some(launcher_path),
            });
        }
        // Different bytes under the full SHA-256 name: not something to paper over.
        Err(Error::StoreConflict {
            path: self.dir.join(format!("{}.raw.js", stems[2])),
        })
    }
}

/// How `path` relates to the bytes we want there. With no bytes to compare
/// (kernel-only import), an existing file counts as `Same`: it is left alone.
fn slot(path: &Path, wanted: Option<&[u8]>) -> Result<Slot> {
    match fs::read(path) {
        Ok(existing) => Ok(match wanted {
            Some(wanted) if existing != wanted => Slot::Taken,
            _ => Slot::Same,
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Slot::Free),
        Err(error) => Err(Error::io(path, error)),
    }
}

/// Create `path` with `bytes`, failing rather than replacing an existing file.
/// The bytes go to a temporary name first so a crash cannot leave a truncated
/// file under a seed name.
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let written = fs::File::create(&temporary)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()))
        .map_err(|error| Error::io(&temporary, error))
        .and_then(|()| match fs::hard_link(&temporary, path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => Err(Error::StoreConflict {
                path: path.to_owned(),
            }),
            Err(error) => Err(Error::io(path, error)),
        });
    let _ = fs::remove_file(&temporary);
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("seed-fetch-store-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn first_save_writes_exactly_two_siblings() {
        let dir = scratch("first");
        let store = SeedStore::new(&dir);
        let seed = SeedToken::new("SEED_a-1").unwrap();
        let outcome = store
            .save(&seed, Some(b"launcher"), b"kernel \xff\x00 bytes")
            .unwrap();
        assert_eq!(outcome.status, StoreStatus::Stored);
        assert_eq!(names(&dir), ["SEED_a-1.async.js", "SEED_a-1.raw.js"]);
        assert_eq!(
            fs::read(outcome.kernel_path).unwrap(),
            b"kernel \xff\x00 bytes"
        );
        assert_eq!(
            fs::read(outcome.launcher_path.unwrap()).unwrap(),
            b"launcher"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn repeat_is_a_duplicate_and_changed_kernel_is_a_variant() {
        let dir = scratch("repeat");
        let store = SeedStore::new(&dir);
        let seed = SeedToken::new("S").unwrap();
        store.save(&seed, Some(b"l1"), b"k1").unwrap();

        let again = store.save(&seed, Some(b"l1"), b"k1").unwrap();
        assert_eq!(
            (again.status, again.launcher_differs),
            (StoreStatus::Duplicate, false)
        );

        // Same kernel behind a launcher with fresh per-request tokens.
        let relaunched = store.save(&seed, Some(b"l2"), b"k1").unwrap();
        assert_eq!(
            (relaunched.status, relaunched.launcher_differs),
            (StoreStatus::Duplicate, true)
        );
        assert_eq!(names(&dir), ["S.async.js", "S.raw.js"]);

        let short = &sha256_hex(b"k2")[..SHORT_DIGEST_LEN];
        let changed = store.save(&seed, Some(b"l3"), b"k2").unwrap();
        assert_eq!(changed.status, StoreStatus::Variant);
        assert_eq!(changed.kernel_path, dir.join(format!("S.{short}.raw.js")));
        assert_eq!(
            changed.launcher_path,
            Some(dir.join(format!("S.{short}.async.js")))
        );
        assert_eq!(
            store.save(&seed, Some(b"l3"), b"k2").unwrap().status,
            StoreStatus::Duplicate
        );

        assert_eq!(names(&dir).len(), 4);
        assert_eq!(fs::read(dir.join("S.raw.js")).unwrap(), b"k1");
        assert_eq!(fs::read(dir.join("S.async.js")).unwrap(), b"l1");
        assert_eq!(fs::read(changed.kernel_path).unwrap(), b"k2");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn kernel_only_import_gains_its_launcher_later() {
        let dir = scratch("kernel-only");
        let store = SeedStore::new(&dir);
        let seed = SeedToken::new("S").unwrap();
        let alone = store.save(&seed, None, b"k1").unwrap();
        assert_eq!(
            (alone.status, alone.launcher_path),
            (StoreStatus::Stored, None)
        );
        assert_eq!(names(&dir), ["S.raw.js"]);

        let paired = store.save(&seed, Some(b"l1"), b"k1").unwrap();
        assert_eq!(paired.status, StoreStatus::Duplicate);
        assert_eq!(paired.launcher_path, Some(dir.join("S.async.js")));
        assert_eq!(
            store.save(&seed, None, b"k1").unwrap().status,
            StoreStatus::Duplicate
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_kernel_is_never_paired_with_a_foreign_launcher() {
        let dir = scratch("orphan");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("S.async.js"), b"someone else's launcher").unwrap();
        let store = SeedStore::new(&dir);
        let outcome = store
            .save(&SeedToken::new("S").unwrap(), Some(b"l1"), b"k1")
            .unwrap();
        assert_eq!(outcome.status, StoreStatus::Variant);
        assert!(!dir.join("S.raw.js").exists());
        assert_eq!(
            fs::read(dir.join("S.async.js")).unwrap(),
            b"someone else's launcher"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn write_new_refuses_to_replace() {
        let dir = scratch("no-replace");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("S.raw.js");
        write_new(&path, b"first").unwrap();
        assert_eq!(
            write_new(&path, b"second").unwrap_err().kind(),
            "store_conflict"
        );
        assert_eq!(fs::read(&path).unwrap(), b"first");
        assert_eq!(names(&dir), ["S.raw.js"]);
        fs::remove_dir_all(dir).unwrap();
    }
}
