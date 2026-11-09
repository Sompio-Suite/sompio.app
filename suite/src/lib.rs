//! What the app needs from the Sompio Lock, in one place: setting it up, unlocking, recovering,
//! and handing each app the key to its own encrypted database. The desktop shell only calls this.

use anyhow::{anyhow, bail, Context, Result};
use sompio_clean::Store;
use sompio_connect::SecretStore;
use sompio_lock::{read_picture, LockFile, PictureSecret, VaultKey};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

/// Where the picture's secret is kept on a PC the person chose to trust.
const REMEMBERED_PICTURE: &str = "lock.picture";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct LockStatus {
    pub set_up: bool,
    pub unlocked: bool,
    /// This PC remembers the key picture, so only the password is needed
    pub remembers_picture: bool,
}

pub struct Vault {
    dir: PathBuf,
    secrets: Arc<dyn SecretStore>,
    key: Mutex<Option<VaultKey>>,
}

impl Vault {
    pub fn new(dir: PathBuf, secrets: Arc<dyn SecretStore>) -> Self {
        Self { dir, secrets, key: Mutex::new(None) }
    }

    pub fn lock_path(&self) -> PathBuf {
        self.dir.join("lock.json")
    }

    pub fn clean_db_path(&self) -> PathBuf {
        self.dir.join("clean.db")
    }

    fn set_key(&self, v: Option<VaultKey>) {
        if let Ok(mut k) = self.key.lock() {
            *k = v;
        }
    }

    fn remembered(&self) -> Option<PictureSecret> {
        self.secrets.get(REMEMBERED_PICTURE).ok().flatten().and_then(|h| PictureSecret::from_hex(&h))
    }

    pub fn status(&self) -> Result<LockStatus> {
        Ok(LockStatus {
            set_up: self.lock_path().exists(),
            unlocked: self.key.lock().map(|k| k.is_some()).unwrap_or(false),
            remembers_picture: self.remembered().is_some(),
        })
    }

    fn picture_from(&self, path: Option<&Path>) -> Result<PictureSecret> {
        match path {
            Some(p) => read_picture(&std::fs::read(p).with_context(|| format!("Couldn't open {}", p.display()))?),
            None => self.remembered().ok_or_else(|| anyhow!("Choose your key picture. This PC doesn't remember it.")),
        }
    }

    fn remember(&self, picture: &PictureSecret, remember: bool) -> Result<()> {
        if remember {
            self.secrets.set(REMEMBERED_PICTURE, &picture.to_hex())
        } else {
            self.secrets.delete(REMEMBERED_PICTURE)
        }
    }

    /// Brings existing app data under the lock: encrypts a plain database in place, or moves aside
    /// one that belongs to a lock that no longer exists (it can't be read by anyone).
    fn secure_data(&self, vault: &VaultKey) -> Result<()> {
        let db = self.clean_db_path();
        if !db.exists() {
            return Ok(());
        }
        let key = vault.derive("clean.db");
        if Store::is_plain(&db) {
            Store::encrypt_existing(&db, &key)?;
            self.remove_old_veil_copy();
        } else if Store::open_encrypted(&db, &key).is_err() {
            let aside = self.dir.join(format!("clean.db.unreadable-{}", sompio_lock::now_ms()));
            std::fs::rename(&db, aside)?;
        }
        Ok(())
    }

    /// The plain copy left behind by the app's old name, now that the data is encrypted here.
    fn remove_old_veil_copy(&self) {
        if let Some(old) = self.dir.parent().map(|p| p.join("app.veil.desktop")) {
            for f in ["veil.db", "veil.db-wal", "veil.db-shm"] {
                let _ = std::fs::remove_file(old.join(f));
            }
        }
    }

    /// First run: creates the lock, writes the key picture where the person chose, and returns the
    /// recovery code to show once.
    pub fn setup(&self, password: &str, picture_path: &Path, remember: bool) -> Result<Zeroizing<String>> {
        if self.lock_path().exists() {
            bail!("Sompio is already locked with a password. Unlock it instead.");
        }
        let s = LockFile::create(password)?;
        std::fs::write(picture_path, &s.picture_png)
            .with_context(|| format!("Couldn't save the key picture to {}", picture_path.display()))?;
        // The lock file is saved before any data is encrypted, so data is never encrypted under a
        // key that isn't written down anywhere.
        s.file.save(&self.lock_path())?;
        self.remember(&s.picture_secret, remember)?;
        self.secure_data(&s.vault)?;
        self.set_key(Some(s.vault.clone()));
        Ok(s.recovery_code)
    }

    pub fn unlock(&self, password: &str, picture_path: Option<&Path>, remember: bool) -> Result<()> {
        let file = LockFile::load(&self.lock_path())?.ok_or_else(|| anyhow!("Sompio has no lock yet."))?;
        let picture = self.picture_from(picture_path)?;
        let vault = file.unlock(password, &picture)?;
        if picture_path.is_some() {
            self.remember(&picture, remember)?;
        }
        self.secure_data(&vault)?;
        self.set_key(Some(vault));
        Ok(())
    }

    /// Recovery code in, new password and new key picture out. Returns the new recovery code.
    pub fn recover(&self, code: &str, new_password: &str, picture_path: &Path, remember: bool) -> Result<Zeroizing<String>> {
        let file = LockFile::load(&self.lock_path())?.ok_or_else(|| anyhow!("Sompio has no lock yet."))?;
        let s = file.recover(code, new_password)?;
        std::fs::write(picture_path, &s.picture_png)
            .with_context(|| format!("Couldn't save the new key picture to {}", picture_path.display()))?;
        s.file.save(&self.lock_path())?;
        self.remember(&s.picture_secret, remember)?;
        self.secure_data(&s.vault)?;
        self.set_key(Some(s.vault.clone()));
        Ok(s.recovery_code)
    }

    pub fn change_password(&self, old: &str, new: &str, picture_path: Option<&Path>) -> Result<()> {
        let mut file = LockFile::load(&self.lock_path())?.ok_or_else(|| anyhow!("Sompio has no lock yet."))?;
        let picture = self.picture_from(picture_path)?;
        file.change_password(old, new, &picture)?;
        file.save(&self.lock_path())
    }

    pub fn forget_picture(&self) -> Result<()> {
        self.secrets.delete(REMEMBERED_PICTURE)
    }

    pub fn lock(&self) {
        self.set_key(None);
    }

    /// Clean's database and its key. Fails while Sompio is locked.
    pub fn clean_db(&self) -> Result<(PathBuf, Zeroizing<[u8; 32]>)> {
        let guard = self.key.lock().map_err(|_| anyhow!("lock state unavailable"))?;
        let vault = guard.as_ref().ok_or_else(|| anyhow!("Sompio is locked. Unlock it to continue."))?;
        Ok((self.clean_db_path(), vault.derive("clean.db")))
    }

    pub fn vault_db_path(&self) -> PathBuf {
        self.dir.join("vault.kdbx")
    }

    /// The password vault's file and key. Fails while Sompio is locked.
    pub fn vault_db(&self) -> Result<(PathBuf, Zeroizing<[u8; 32]>)> {
        let guard = self.key.lock().map_err(|_| anyhow!("lock state unavailable"))?;
        let vault = guard.as_ref().ok_or_else(|| anyhow!("Sompio is locked. Unlock it to continue."))?;
        Ok((self.vault_db_path(), vault.derive("vault.kdbx")))
    }

    pub fn calendar_db_path(&self) -> PathBuf {
        self.dir.join("calendar.db")
    }

    /// Calendar's database and its key. Fails while Sompio is locked.
    pub fn calendar_db(&self) -> Result<(PathBuf, Zeroizing<[u8; 32]>)> {
        let guard = self.key.lock().map_err(|_| anyhow!("lock state unavailable"))?;
        let vault = guard.as_ref().ok_or_else(|| anyhow!("Sompio is locked. Unlock it to continue."))?;
        Ok((self.calendar_db_path(), vault.derive("calendar.db")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sompio_connect::MemoryStore;

    const PW: &str = "fell lantern quiet 42";

    fn vault(dir: &Path) -> Vault {
        Vault::new(dir.to_path_buf(), Arc::new(MemoryStore::default()))
    }

    #[test]
    fn setup_encrypts_existing_data_and_unlock_flows() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("app.sompio.desktop");
        std::fs::create_dir_all(&dir).unwrap();
        // Data from before the lock, plus the old app's plain copy
        {
            let mut s = Store::open(&dir.join("clean.db")).unwrap();
            sompio_clean::load_demo(&mut s).unwrap();
        }
        let old = tmp.path().join("app.veil.desktop");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::copy(dir.join("clean.db"), old.join("veil.db")).unwrap();

        let v = vault(&dir);
        assert_eq!(v.status().unwrap(), LockStatus { set_up: false, unlocked: false, remembers_picture: false });
        assert!(v.clean_db().is_err());
        let pic = tmp.path().join("Sompio key.png");
        let code = v.setup(PW, &pic, true).unwrap();
        assert!(pic.exists());
        assert!(!Store::is_plain(&dir.join("clean.db")), "existing data is encrypted");
        assert!(!old.join("veil.db").exists(), "old plain copy removed");
        let (path, key) = v.clean_db().unwrap();
        assert_ne!(*v.calendar_db().unwrap().1, *key, "each app has its own key");
        assert!(Store::open_encrypted(&path, &key).unwrap().has_data().unwrap());
        assert!(v.setup(PW, &pic, true).is_err(), "setup only once");

        // Lock, then unlock with the password alone (picture remembered)
        v.lock();
        assert!(v.clean_db().is_err());
        assert!(v.unlock("wrong password here", None, true).is_err());
        v.unlock(PW, None, true).unwrap();
        assert!(v.status().unwrap().unlocked);

        // A new PC: nothing remembered, so the picture is needed
        let fresh = vault(&dir);
        assert!(fresh.unlock(PW, None, false).unwrap_err().to_string().contains("Choose your key picture"));
        fresh.unlock(PW, Some(&pic), false).unwrap();
        assert!(!fresh.status().unwrap().remembers_picture);

        // Recovery: new password and picture, same data
        let fresh2 = vault(&dir);
        let pic2 = tmp.path().join("Sompio key 2.png");
        let code2 = fresh2.recover(&code, "brand new phrase here", &pic2, true).unwrap();
        assert_ne!(code2.as_str(), code.as_str());
        let (path, key) = fresh2.clean_db().unwrap();
        assert!(Store::open_encrypted(&path, &key).unwrap().has_data().unwrap());
        assert!(vault(&dir).unlock(PW, Some(&pic), false).is_err(), "old password and picture stop working");
        vault(&dir).unlock("brand new phrase here", Some(&pic2), false).unwrap();
    }

    #[test]
    fn data_from_a_lost_lock_is_moved_aside() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        {
            let mut s = Store::open(&dir.join("clean.db")).unwrap();
            sompio_clean::load_demo(&mut s).unwrap();
        }
        Store::encrypt_existing(&dir.join("clean.db"), &[9u8; 32]).unwrap();
        let v = vault(&dir);
        v.setup(PW, &dir.join("k.png"), false).unwrap();
        assert!(!dir.join("clean.db").exists());
        assert!(std::fs::read_dir(&dir).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with("clean.db.unreadable-")));
    }
}
