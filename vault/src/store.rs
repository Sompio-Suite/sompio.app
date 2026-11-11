//! The vault itself: one KDBX 4 file, the same format KeePass and KeePassXC use. Inside Sompio
//! it's opened with a key from the Sompio Lock; exports get a password of your choosing.

use anyhow::{anyhow, bail, Context, Result};
use chrono::{NaiveDate, NaiveDateTime, Utc};
use keepass::config::{DatabaseConfig, KdfConfig};
use keepass::db::{fields, AutoType, AutoTypeAssociation, Entry, EntryId, GroupId, Times, Value};
use keepass::{Database, DatabaseKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zeroize::Zeroizing;

/// KeePassXC keeps 2FA settings in this field, as an otpauth:// link.
pub const OTP: &str = fields::OTP;
pub const DEFAULT_SEQUENCE: &str = "{USERNAME}{TAB}{PASSWORD}{ENTER}";
const RECYCLE_BIN_ICON: usize = 43;

#[derive(Debug, Clone, Serialize)]
pub struct GroupView {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub depth: usize,
    /// Entries directly in this folder
    pub entries: usize,
    pub recycle_bin: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntrySummary {
    pub id: String,
    pub group_id: String,
    pub group_path: String,
    pub title: String,
    pub username: String,
    pub url: String,
    pub tags: Vec<String>,
    pub has_password: bool,
    pub has_totp: bool,
    pub expires: Option<String>,
    pub expired: bool,
    pub modified: Option<String>,
    pub in_recycle_bin: bool,
    /// The entry's own icon (a downloaded favicon), by its id in `icons()`
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldView {
    pub key: String,
    /// Hidden fields come back empty until revealed
    pub value: Option<String>,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoTypeSettings {
    pub enabled: bool,
    /// Empty: the usual {USERNAME}{TAB}{PASSWORD}{ENTER}
    pub sequence: String,
    /// Window titles this entry is for, * as a wildcard
    pub windows: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryView {
    pub index: usize,
    pub modified: Option<String>,
    pub title: String,
    pub username: String,
    pub password_changed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttachmentView {
    pub name: String,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MergeSummary {
    pub added: usize,
    pub updated: usize,
    pub moved: usize,
    pub deleted: usize,
    pub warnings: Vec<String>,
}

/// Files kept inside an entry are limited, so the vault stays quick to open and save.
pub const MAX_ATTACHMENT: usize = 25 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct EntryDetail {
    #[serde(flatten)]
    pub summary: EntrySummary,
    pub notes: String,
    pub custom: Vec<FieldView>,
    pub autotype: AutoTypeSettings,
    pub created: Option<String>,
    pub password_modified: Option<String>,
    pub history: Vec<HistoryView>,
    pub attachments: Vec<AttachmentView>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldInput {
    pub key: String,
    /// None keeps what's there (a hidden value the person didn't reveal)
    pub value: Option<String>,
    #[serde(default)]
    pub protected: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EntryInput {
    pub group_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub username: String,
    /// None keeps the current password
    pub password: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// YYYY-MM-DD, or None for never
    pub expires: Option<String>,
    #[serde(default)]
    pub custom: Vec<FieldInput>,
    /// None keeps it, "" removes it. An otpauth:// link or a bare secret.
    pub otp: Option<String>,
    pub autotype: Option<AutoTypeSettings>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ImportSummary {
    pub entries: usize,
    pub groups: usize,
    pub skipped_attachments: usize,
    pub group_id: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Totp {
    pub code: String,
    pub remaining: u64,
    pub period: u64,
}

fn iso(t: Option<NaiveDateTime>) -> Option<String> {
    t.map(|t| t.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

pub(crate) fn field<'a>(e: &'a Entry, key: &str) -> &'a str {
    e.get(key).unwrap_or("")
}

fn parse_entry_id(id: &str) -> Result<EntryId> {
    Ok(EntryId::from_uuid(Uuid::parse_str(id).map_err(|_| anyhow!("That entry no longer exists."))?))
}

fn parse_group_id(id: &str) -> Result<GroupId> {
    Ok(GroupId::from_uuid(Uuid::parse_str(id).map_err(|_| anyhow!("That folder no longer exists."))?))
}

/// A bare base32 secret becomes an otpauth link, the way KeePassXC stores it.
pub fn otp_link(raw: &str, title: &str, username: &str) -> String {
    let raw = raw.trim();
    if raw.to_lowercase().starts_with("otpauth://") {
        return raw.to_string();
    }
    let secret: String = raw.chars().filter(|c| !c.is_whitespace() && *c != '-').collect::<String>().to_uppercase();
    let label = if username.is_empty() { title.to_string() } else { format!("{title}:{username}") };
    let enc = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    format!("otpauth://totp/{}?secret={secret}&period=30&digits=6&issuer={}", enc(&label), enc(title))
}

pub struct Vault {
    db: Database,
    path: PathBuf,
    key: Zeroizing<[u8; 32]>,
}

/// The lock's key, given to KeePass as a 32-byte key file.
fn inner_key(key: &[u8; 32]) -> DatabaseKey {
    let mut bytes: &[u8] = key;
    DatabaseKey::new().with_keyfile(&mut bytes).expect("reading from memory")
}

/// The lock already stretched the password, so the vault file's own KDF can be light.
fn inner_config() -> DatabaseConfig {
    let mut c = DatabaseConfig::default();
    c.kdf_config = KdfConfig::Argon2id { iterations: 2, memory: 8 * 1024 * 1024, parallelism: 2, version: argon2::Version::Version13 };
    c
}

/// Exports protect themselves with only a password, so they get KeePassXC-strength settings.
fn export_kdf() -> KdfConfig {
    #[cfg(test)]
    let (iterations, memory) = (1, 1024 * 1024);
    #[cfg(not(test))]
    let (iterations, memory) = (8, 64 * 1024 * 1024);
    KdfConfig::Argon2id { iterations, memory, parallelism: 2, version: argon2::Version::Version13 }
}

pub fn user_key(password: &str, keyfile: Option<&[u8]>) -> Result<DatabaseKey> {
    let mut k = DatabaseKey::new();
    if !password.is_empty() {
        k = k.with_password(password);
    }
    if let Some(mut f) = keyfile {
        k = k.with_keyfile(&mut f)?;
    }
    if password.is_empty() && keyfile.is_none() {
        bail!("Enter the file's password, or choose its key file.");
    }
    Ok(k)
}

impl Vault {
    /// Opens the vault file, or starts a new one if there isn't one yet.
    pub fn open(path: &Path, key: &[u8; 32]) -> Result<Self> {
        let db = if path.exists() {
            let bytes = std::fs::read(path).with_context(|| format!("Couldn't read {}", path.display()))?;
            Database::parse(&bytes, inner_key(key)).map_err(|_| anyhow!("The vault didn't open with this key."))?
        } else {
            let mut db = Database::with_config(inner_config());
            db.root_mut().name = "Vault".into();
            db.meta.database_name = Some("Sompio Vault".into());
            db.meta.generator = Some("Sompio".into());
            db
        };
        let mut v = Self { db, path: path.to_path_buf(), key: Zeroizing::new(*key) };
        v.ensure_recycle_bin();
        if !path.exists() {
            v.save()?;
        }
        Ok(v)
    }

    pub fn database(&self) -> &Database {
        &self.db
    }

    /// Writes a new file next to the old one and swaps them, keeping the previous copy as .bak.
    pub fn save(&self) -> Result<()> {
        let tmp = self.path.with_extension("kdbx.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            self.db.save(&mut f, inner_key(&self.key)).map_err(|e| anyhow!("Couldn't save the vault: {e}"))?;
            f.sync_all()?;
        }
        // Check the new file opens before replacing anything.
        Database::parse(&std::fs::read(&tmp)?, inner_key(&self.key)).map_err(|e| anyhow!("The saved vault didn't read back ({e}), so nothing was replaced."))?;
        if self.path.exists() {
            let _ = std::fs::copy(&self.path, self.path.with_extension("kdbx.bak"));
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    fn ensure_recycle_bin(&mut self) {
        if self.db.recycle_bin().is_some() {
            return;
        }
        let mut root = self.db.root_mut();
        let mut bin = root.add_group();
        bin.name = "Recycle Bin".into();
        bin.set_icon_builtin(RECYCLE_BIN_ICON);
        bin.enable_autotype = Some(false);
        bin.enable_searching = Some(false);
        let id = bin.id();
        self.db.meta.recyclebin_enabled = Some(true);
        self.db.meta.recyclebin_uuid = Some(id.uuid());
        self.db.meta.recyclebin_changed = Some(Times::now());
    }

    fn bin_id(&self) -> Option<GroupId> {
        self.db.recycle_bin().map(|g| g.id())
    }

    /// True if the group is the recycle bin or inside it.
    fn in_bin(&self, mut group: GroupId) -> bool {
        let Some(bin) = self.bin_id() else { return false };
        loop {
            if group == bin {
                return true;
            }
            match self.db.group(group).and_then(|g| g.parent().map(|p| p.id())) {
                Some(p) => group = p,
                None => return false,
            }
        }
    }

    fn group_path(&self, group: GroupId) -> String {
        let mut names = Vec::new();
        let mut cur = Some(group);
        while let Some(id) = cur {
            let Some(g) = self.db.group(id) else { break };
            let Some(parent) = g.parent().map(|p| p.id()) else { break };
            names.push(g.name.clone());
            cur = Some(parent);
        }
        names.reverse();
        names.join(" / ")
    }

    pub fn groups(&self) -> Vec<GroupView> {
        let mut out = Vec::new();
        fn walk(v: &Vault, g: keepass::db::GroupRef<'_>, depth: usize, out: &mut Vec<GroupView>) {
            out.push(GroupView {
                id: g.id().uuid().to_string(),
                parent: g.parent().map(|p| p.id().uuid().to_string()),
                name: g.name.clone(),
                depth,
                entries: g.entries().count(),
                recycle_bin: Some(g.id()) == v.bin_id(),
            });
            let mut kids: Vec<_> = g.groups().collect();
            // The recycle bin goes last, the rest alphabetically
            kids.sort_by_key(|k| (Some(k.id()) == v.bin_id(), k.name.to_lowercase()));
            for k in kids {
                walk(v, k, depth + 1, out);
            }
        }
        walk(self, self.db.root(), 0, &mut out);
        out
    }

    fn summary(&self, e: &keepass::db::EntryRef<'_>) -> EntrySummary {
        let group = e.parent().id();
        let now = Utc::now().naive_utc();
        let expires = (e.times.expires == Some(true)).then_some(e.times.expiry);
        EntrySummary {
            id: e.id().uuid().to_string(),
            group_id: group.uuid().to_string(),
            group_path: self.group_path(group),
            title: field(e, fields::TITLE).to_string(),
            username: field(e, fields::USERNAME).to_string(),
            url: field(e, fields::URL).to_string(),
            tags: e.tags.clone(),
            has_password: !field(e, fields::PASSWORD).is_empty(),
            has_totp: !field(e, OTP).is_empty() || e.get("TOTP Seed").is_some(),
            expires: iso(expires),
            expired: expires.is_some_and(|x| x <= now),
            modified: iso(e.times.last_modification),
            in_recycle_bin: self.in_bin(group),
            icon: e.custom_icon().map(|c| c.id().uuid().to_string()),
        }
    }

    /// Entries in a folder (and its sub-folders), or everywhere; filtered by a search.
    pub fn entries(&self, group: Option<&str>, search: &str) -> Result<Vec<EntrySummary>> {
        let scope = group.map(parse_group_id).transpose()?;
        let terms: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
        let mut out = Vec::new();
        for e in self.db.iter_all_entries() {
            let gid = e.parent().id();
            let in_bin = self.in_bin(gid);
            match scope {
                Some(s) => {
                    let mut cur = Some(gid);
                    let mut inside = false;
                    while let Some(c) = cur {
                        if c == s {
                            inside = true;
                            break;
                        }
                        cur = self.db.group(c).and_then(|g| g.parent().map(|p| p.id()));
                    }
                    if !inside || (in_bin && Some(s) != self.bin_id() && !self.in_bin(s)) {
                        continue;
                    }
                }
                None if in_bin => continue,
                None => {}
            }
            if !terms.is_empty() {
                let hay = format!(
                    "{} {} {} {} {} {}",
                    field(&e, fields::TITLE),
                    field(&e, fields::USERNAME),
                    field(&e, fields::URL),
                    field(&e, fields::NOTES),
                    e.tags.join(" "),
                    self.group_path(gid)
                )
                .to_lowercase();
                if !terms.iter().all(|t| hay.contains(t.as_str())) {
                    continue;
                }
            }
            out.push(self.summary(&e));
        }
        out.sort_by_key(|s| (s.title.to_lowercase(), s.username.to_lowercase()));
        Ok(out)
    }

    fn entry_ref(&self, id: &str) -> Result<keepass::db::EntryRef<'_>> {
        self.db.entry(parse_entry_id(id)?).ok_or_else(|| anyhow!("That entry no longer exists."))
    }

    pub fn entry(&self, id: &str) -> Result<EntryDetail> {
        let e = self.entry_ref(id)?;
        let custom = e
            .fields
            .iter()
            .filter(|(k, _)| !fields::KNOWN_FIELDS.contains(&k.as_str()) && k.as_str() != OTP)
            .map(|(k, v)| FieldView { key: k.clone(), value: (!v.is_protected()).then(|| v.get().clone()), protected: v.is_protected() })
            .collect::<Vec<_>>();
        let mut custom = custom;
        custom.sort_by(|a, b| a.key.cmp(&b.key));
        let at = e.autotype.clone().unwrap_or(AutoType { enabled: true, ..Default::default() });
        let history: Vec<HistoryView> = e
            .history
            .as_ref()
            .map(|h| {
                let list = h.get_entries();
                list.iter()
                    .enumerate()
                    .map(|(i, old)| {
                        // History is newest first; the password changed if it differs from the version after it
                        let newer = if i == 0 { field(&e, fields::PASSWORD) } else { field(&list[i - 1], fields::PASSWORD) };
                        HistoryView {
                            index: i,
                            modified: iso(old.times.last_modification),
                            title: field(old, fields::TITLE).to_string(),
                            username: field(old, fields::USERNAME).to_string(),
                            password_changed: field(old, fields::PASSWORD) != newer,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let password_modified = self.password_modified(&e);
        let mut attachments: Vec<AttachmentView> = e.attachments_named().map(|(n, a)| AttachmentView { name: n.to_string(), size: a.data.get().len() }).collect();
        attachments.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(EntryDetail {
            summary: self.summary(&e),
            notes: field(&e, fields::NOTES).to_string(),
            custom,
            autotype: AutoTypeSettings {
                enabled: at.enabled,
                sequence: at.default_sequence.unwrap_or_default(),
                windows: at.associations.iter().map(|a| a.window.clone()).collect(),
            },
            created: iso(e.times.creation),
            password_modified: iso(password_modified),
            history,
            attachments,
        })
    }

    /// When the password last changed: the newest history version with a different password.
    pub(crate) fn password_modified(&self, e: &Entry) -> Option<NaiveDateTime> {
        let pw = field(e, fields::PASSWORD);
        let mut when = e.times.last_modification.or(e.times.creation);
        if let Some(h) = &e.history {
            for old in h.get_entries() {
                if field(old, fields::PASSWORD) != pw {
                    break;
                }
                when = old.times.last_modification.or(when);
            }
        }
        when
    }

    /// One field's value, for copying, showing or typing. Counts as a use of the entry.
    pub fn reveal(&mut self, id: &str, key: &str) -> Result<Zeroizing<String>> {
        let eid = parse_entry_id(id)?;
        let v = {
            let e = self.db.entry(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?;
            Zeroizing::new(field(&e, key).to_string())
        };
        if let Some(mut e) = self.db.entry_mut(eid) {
            e.times.last_access = Some(Times::now());
            e.times.usage_count = Some(e.times.usage_count.unwrap_or(0) + 1);
        }
        Ok(v)
    }

    /// A field from an older version.
    pub fn reveal_history(&self, id: &str, index: usize, key: &str) -> Result<Zeroizing<String>> {
        let e = self.entry_ref(id)?;
        let h = e.history.as_ref().and_then(|h| h.get_entries().get(index)).ok_or_else(|| anyhow!("That version no longer exists."))?;
        Ok(Zeroizing::new(field(h, key).to_string()))
    }

    pub fn totp(&self, id: &str) -> Result<Option<Totp>> {
        let e = self.entry_ref(id)?;
        let raw = match (e.get(OTP), e.get("TOTP Seed")) {
            (Some(link), _) if !link.is_empty() => link.to_string(),
            (_, Some(seed)) => otp_link(seed, field(&e, fields::TITLE), ""),
            _ => return Ok(None),
        };
        let t: keepass::db::TOTP = raw.parse().map_err(|_| anyhow!("The 2FA setup on this entry can't be read."))?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
        let c = t.value_at(now);
        Ok(Some(Totp { code: c.code, remaining: c.valid_for.as_secs(), period: c.period.as_secs() }))
    }

    fn apply(e: &mut Entry, input: &EntryInput) -> Result<()> {
        if input.title.trim().is_empty() {
            bail!("Give the entry a title.");
        }
        e.set_unprotected(fields::TITLE, input.title.trim());
        e.set_unprotected(fields::USERNAME, input.username.as_str());
        e.set_unprotected(fields::URL, input.url.trim());
        e.set_unprotected(fields::NOTES, input.notes.as_str());
        if let Some(pw) = &input.password {
            e.set_protected(fields::PASSWORD, pw.as_str());
        }
        let mut tags: Vec<String> = input.tags.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        tags.dedup();
        e.tags = tags;
        match input.expires.as_deref().filter(|x| !x.is_empty()) {
            Some(d) => {
                let d = NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| anyhow!("\"{d}\" isn't a date."))?;
                e.times.expires = Some(true);
                e.times.expiry = d.and_hms_opt(0, 0, 0).unwrap_or_default();
            }
            None => e.times.expires = Some(false),
        }
        // Your own fields: replace the set, keeping hidden values that weren't revealed
        let reserved: HashSet<&str> = fields::KNOWN_FIELDS.iter().copied().chain([OTP]).collect();
        let mut keep: HashMap<String, Value<String>> = HashMap::new();
        for f in &input.custom {
            let k = f.key.trim();
            if k.is_empty() {
                continue;
            }
            if reserved.contains(k) {
                bail!("\"{k}\" is a built-in field name. Pick another name.");
            }
            let value = match &f.value {
                Some(v) => v.clone(),
                None => e.get(k).map(str::to_string).unwrap_or_default(),
            };
            keep.insert(k.to_string(), if f.protected { Value::protected(value) } else { Value::unprotected(value) });
        }
        e.fields.retain(|k, _| reserved.contains(k.as_str()));
        e.fields.extend(keep);
        if let Some(otp) = &input.otp {
            if otp.trim().is_empty() {
                e.fields.remove(OTP);
            } else {
                let link = otp_link(otp, input.title.trim(), &input.username);
                link.parse::<keepass::db::TOTP>().map_err(|_| anyhow!("That 2FA secret can't be read. Paste the setup key or the otpauth:// link."))?;
                e.set_protected(OTP, link);
            }
        }
        if let Some(a) = &input.autotype {
            let seq = a.sequence.trim();
            if !seq.is_empty() {
                crate::autotype::parse(seq).map_err(|m| anyhow!("Auto-type sequence: {m}"))?;
            }
            e.autotype = Some(AutoType {
                enabled: a.enabled,
                default_sequence: (!seq.is_empty()).then(|| seq.to_string()),
                associations: a.windows.iter().map(|w| w.trim()).filter(|w| !w.is_empty()).map(|w| AutoTypeAssociation { window: w.to_string(), sequence: String::new() }).collect(),
                ..Default::default()
            });
        }
        Ok(())
    }

    fn target_group(&self, id: Option<&str>) -> Result<GroupId> {
        match id {
            Some(g) => {
                let gid = parse_group_id(g)?;
                self.db.group(gid).ok_or_else(|| anyhow!("That folder no longer exists."))?;
                Ok(gid)
            }
            None => Ok(self.db.root().id()),
        }
    }

    pub fn add_entry(&mut self, input: &EntryInput) -> Result<String> {
        let gid = self.target_group(input.group_id.as_deref())?;
        // Check everything on a scratch copy first, so a bad field leaves nothing half-made
        let mut scratch = Database::new();
        {
            let mut root = scratch.root_mut();
            Self::apply(&mut root.add_entry(), input)?;
        }
        let mut g = self.db.group_mut(gid).ok_or_else(|| anyhow!("That folder no longer exists."))?;
        let mut e = g.add_entry();
        Self::apply(&mut e, input)?;
        let id = e.id().uuid().to_string();
        self.save()?;
        Ok(id)
    }

    /// Edits keep the previous version in the entry's history, like KeePassXC.
    pub fn update_entry(&mut self, id: &str, input: &EntryInput) -> Result<()> {
        let eid = parse_entry_id(id)?;
        let mut scratch = Database::new();
        {
            let mut root = scratch.root_mut();
            let mut s = root.add_entry();
            if let Some(cur) = self.db.entry(eid) {
                s.fields = cur.fields.clone();
            }
            Self::apply(&mut s, input)?;
        }
        let gid = input.group_id.as_deref().map(|g| self.target_group(Some(g))).transpose()?;
        let mut err = None;
        {
            let mut e = self.db.entry_mut(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?;
            e.edit_tracking(|t| {
                if let Err(x) = Self::apply(t, input) {
                    err = Some(x);
                }
                if let Some(g) = gid {
                    if let Err(x) = t.move_to(g) {
                        err = Some(anyhow!("{x}"));
                    }
                }
            });
        }
        if let Some(e) = err {
            return Err(e);
        }
        self.trim_history(eid);
        self.save()
    }

    /// Keeps the last 10 versions, as KeePassXC does by default.
    fn trim_history(&mut self, eid: EntryId) {
        let max = self.db.meta.history_max_items.filter(|m| *m > 0).unwrap_or(10) as usize;
        if let Some(mut e) = self.db.entry_mut(eid) {
            if let Some(h) = e.history.as_mut() {
                if h.get_entries().len() > max {
                    let kept: Vec<Entry> = h.get_entries()[..max].to_vec();
                    let mut nh = keepass::db::History::default();
                    for old in kept.into_iter().rev() {
                        nh.add_entry(old);
                    }
                    e.history = Some(nh);
                }
            }
        }
        self.db.remove_unused_attachments();
    }

    /// Puts an older version back. The current one goes into the history.
    pub fn restore_version(&mut self, id: &str, index: usize) -> Result<()> {
        let eid = parse_entry_id(id)?;
        let old = {
            let e = self.db.entry(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?;
            e.history.as_ref().and_then(|h| h.get_entries().get(index).cloned()).ok_or_else(|| anyhow!("That version no longer exists."))?
        };
        {
            let mut e = self.db.entry_mut(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?;
            e.edit_tracking(|t| {
                t.fields = old.fields.clone();
                t.tags = old.tags.clone();
                t.autotype = old.autotype.clone();
                t.times.expires = old.times.expires;
                t.times.expiry = old.times.expiry;
            });
        }
        self.trim_history(eid);
        self.save()
    }

    /// The first delete moves to the recycle bin; deleting from the bin removes it for good.
    pub fn delete_entry(&mut self, id: &str) -> Result<bool> {
        let eid = parse_entry_id(id)?;
        let gid = self.db.entry(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?.parent().id();
        let forever = self.in_bin(gid);
        if forever {
            self.db.entry_mut(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?.track_changes().remove();
            self.db.remove_unused_attachments();
        } else {
            self.ensure_recycle_bin();
            let bin = self.bin_id().ok_or_else(|| anyhow!("No recycle bin"))?;
            self.db.entry_mut(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?.move_to(bin).map_err(|e| anyhow!("{e}"))?;
            if let Some(mut e) = self.db.entry_mut(eid) {
                e.times.location_changed = Some(Times::now());
            }
        }
        self.save()?;
        Ok(forever)
    }

    /// Moves an entry out of the recycle bin, back where it was (or to the top folder).
    pub fn restore_entry(&mut self, id: &str) -> Result<()> {
        let eid = parse_entry_id(id)?;
        let back = {
            let e = self.db.entry(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?;
            e.previous_parent().map(|g| g.id()).filter(|g| !self.in_bin(*g)).unwrap_or(self.db.root().id())
        };
        self.db.entry_mut(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?.move_to(back).map_err(|e| anyhow!("{e}"))?;
        self.save()
    }

    /// A copy of an entry in the same folder, "Title - Clone", like KeePassXC's Clone (no history).
    pub fn clone_entry(&mut self, id: &str) -> Result<String> {
        let eid = parse_entry_id(id)?;
        let icon = self.db.entry(eid).and_then(|e| e.custom_icon().map(|c| c.id()));
        let (gid, fields_, tags, autotype, expires, expiry, files) = {
            let e = self.db.entry(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?;
            let files: Vec<(String, Value<Vec<u8>>)> = e.attachments_named().map(|(n, a)| (n.to_string(), a.data.clone())).collect();
            (e.parent().id(), e.fields.clone(), e.tags.clone(), e.autotype.clone(), e.times.expires, e.times.expiry, files)
        };
        let title = format!("{} - Clone", fields_.get(fields::TITLE).map(|v| v.get().to_string()).unwrap_or_default());
        let new_id = {
            let mut g = self.db.group_mut(gid).ok_or_else(|| anyhow!("That folder no longer exists."))?;
            let mut n = g.add_entry();
            n.fields = fields_;
            n.tags = tags;
            n.autotype = autotype;
            n.times.expires = expires;
            n.times.expiry = expiry;
            n.set_unprotected(fields::TITLE, title.as_str());
            for (name, data) in files {
                n.add_attachment(name, data);
            }
            if let Some(i) = icon {
                let _ = n.set_icon_custom(i);
            }
            n.id().uuid().to_string()
        };
        self.save()?;
        Ok(new_id)
    }

    /// Sets (or with None, removes) an entry's own icon. `png` should come from `icons::to_png`.
    /// Entries for the same website share one stored icon; icons nothing uses any more go.
    pub fn set_icon(&mut self, id: &str, png: Option<Vec<u8>>) -> Result<Option<String>> {
        let eid = parse_entry_id(id)?;
        self.db.entry(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?;
        let same = png.as_ref().and_then(|p| self.db.iter_all_custom_icons().find(|c| c.data == *p).map(|c| c.id()));
        let new_id = {
            let mut e = self.db.entry_mut(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?;
            let id = match (png, same) {
                (None, _) => {
                    e.set_icon_none();
                    None
                }
                (Some(_), Some(existing)) => {
                    e.set_icon_custom(existing).map_err(|x| anyhow!("{x}"))?;
                    Some(existing)
                }
                (Some(p), None) => Some(e.set_icon_custom_new(p).as_ref().id()),
            };
            e.times.last_modification = Some(Times::now());
            id
        };
        self.drop_unused_icons();
        self.save()?;
        Ok(new_id.map(|i| i.uuid().to_string()))
    }

    fn drop_unused_icons(&mut self) {
        let unused: Vec<keepass::db::CustomIconId> =
            self.db.iter_all_custom_icons().filter(|c| c.entries(true).next().is_none() && c.groups().next().is_none()).map(|c| c.id()).collect();
        for id in unused {
            if let Some(c) = self.db.custom_icon_mut(id) {
                c.remove();
            }
        }
    }

    /// Every entry icon, as id -> "data:image/png;base64,..." for the window.
    pub fn icons(&self) -> std::collections::HashMap<String, String> {
        self.db.iter_all_custom_icons().map(|c| (c.id().uuid().to_string(), crate::icons::data_url(&c.data))).collect()
    }

    /// Entries with a website and no icon yet, for "Get icons for all entries".
    pub fn entries_without_icon(&self) -> Vec<(String, String)> {
        self.db
            .iter_all_entries()
            .filter(|e| !self.in_bin(e.parent().id()) && e.custom_icon().is_none())
            .map(|e| (e.id().uuid().to_string(), field(&e, fields::URL).trim().to_string()))
            .filter(|(_, u)| !u.is_empty() && crate::icons::site_root(u).is_ok())
            .collect()
    }

    pub fn move_entry(&mut self, id: &str, group: &str) -> Result<()> {
        let eid = parse_entry_id(id)?;
        let gid = self.target_group(Some(group))?;
        self.db.entry_mut(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?.move_to(gid).map_err(|e| anyhow!("{e}"))?;
        self.save()
    }

    pub fn empty_recycle_bin(&mut self) -> Result<usize> {
        let Some(bin) = self.bin_id() else { return Ok(0) };
        let entries: Vec<EntryId> = self.db.iter_all_entries().filter(|e| self.in_bin(e.parent().id())).map(|e| e.id()).collect();
        let groups: Vec<GroupId> = self.db.group(bin).map(|g| g.groups().map(|k| k.id()).collect()).unwrap_or_default();
        for e in &entries {
            if let Some(mut m) = self.db.entry_mut(*e) {
                m.track_changes().remove();
            }
        }
        for g in groups {
            if let Some(mut m) = self.db.group_mut(g) {
                let _ = m.track_changes().remove();
            }
        }
        self.db.remove_unused_attachments();
        self.save()?;
        Ok(entries.len())
    }

    pub fn add_group(&mut self, parent: Option<&str>, name: &str) -> Result<String> {
        if name.trim().is_empty() {
            bail!("Give the folder a name.");
        }
        let pid = self.target_group(parent)?;
        if self.in_bin(pid) {
            bail!("Folders can't be made inside the recycle bin.");
        }
        let mut p = self.db.group_mut(pid).ok_or_else(|| anyhow!("That folder no longer exists."))?;
        let mut g = p.add_group();
        g.name = name.trim().into();
        let id = g.id().uuid().to_string();
        self.save()?;
        Ok(id)
    }

    pub fn rename_group(&mut self, id: &str, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            bail!("Give the folder a name.");
        }
        let gid = parse_group_id(id)?;
        let mut g = self.db.group_mut(gid).ok_or_else(|| anyhow!("That folder no longer exists."))?;
        g.name = name.trim().into();
        g.times.last_modification = Some(Times::now());
        self.save()
    }

    /// Folders go to the recycle bin with everything in them.
    pub fn delete_group(&mut self, id: &str) -> Result<()> {
        let gid = parse_group_id(id)?;
        if gid == self.db.root().id() || Some(gid) == self.bin_id() {
            bail!("This folder can't be removed.");
        }
        if self.in_bin(gid) {
            self.db.group_mut(gid).ok_or_else(|| anyhow!("That folder no longer exists."))?.track_changes().remove().map_err(|e| anyhow!("{e}"))?;
            self.db.remove_unused_attachments();
        } else {
            self.ensure_recycle_bin();
            let bin = self.bin_id().ok_or_else(|| anyhow!("No recycle bin"))?;
            self.db.group_mut(gid).ok_or_else(|| anyhow!("That folder no longer exists."))?.move_to(bin).map_err(|e| anyhow!("{e}"))?;
        }
        self.save()
    }

    pub fn move_group(&mut self, id: &str, parent: &str) -> Result<()> {
        let gid = parse_group_id(id)?;
        let pid = self.target_group(Some(parent))?;
        if gid == self.db.root().id() || Some(gid) == self.bin_id() {
            bail!("This folder can't be moved.");
        }
        self.db.group_mut(gid).ok_or_else(|| anyhow!("That folder no longer exists."))?.move_to(pid).map_err(|_| anyhow!("A folder can't go inside itself."))?;
        self.save()
    }

    /// Keeps a file inside the entry (encrypted with the rest of the vault). Same name replaces.
    pub fn add_attachment(&mut self, id: &str, name: &str, data: Vec<u8>) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            bail!("The file needs a name.");
        }
        if data.len() > MAX_ATTACHMENT {
            bail!("That file is over 25 MB. Keep big files elsewhere and note where in the entry.");
        }
        let eid = parse_entry_id(id)?;
        {
            let mut e = self.db.entry_mut(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?;
            e.edit_tracking(|t| {
                t.add_attachment(name.to_string(), Value::protected(data));
            });
        }
        self.trim_history(eid);
        self.save()
    }

    pub fn remove_attachment(&mut self, id: &str, name: &str) -> Result<()> {
        let eid = parse_entry_id(id)?;
        {
            let mut e = self.db.entry_mut(eid).ok_or_else(|| anyhow!("That entry no longer exists."))?;
            e.edit_tracking(|t| t.as_mut().remove_attachment_by_name(name));
        }
        self.trim_history(eid);
        self.save()
    }

    pub fn attachment(&self, id: &str, name: &str) -> Result<Zeroizing<Vec<u8>>> {
        let e = self.entry_ref(id)?;
        let a = e.attachment_by_name(name).ok_or_else(|| anyhow!("That file is no longer in this entry."))?;
        Ok(Zeroizing::new(a.data.get().clone()))
    }

    /// Brings in changes from another copy of this vault (one you exported and edited in
    /// KeePassXC, say). Entries are matched by their ID; the newer version of each wins and the
    /// older one goes into its history. A KeePass file that isn't a copy of this vault comes in
    /// as new entries.
    pub fn merge_kdbx(&mut self, bytes: &[u8], password: &str, keyfile: Option<&[u8]>) -> Result<MergeSummary> {
        let other = Database::parse(bytes, user_key(password, keyfile)?).map_err(|e| match e {
            keepass::db::DatabaseOpenError::Key(_) => anyhow!("The password or key file isn't right for this file."),
            other => anyhow!("This file couldn't be opened as a KeePass database: {other}"),
        })?;
        // A different database has a different top folder: bring it in as its own folder.
        if other.root().id() != self.db.root().id() {
            let label = other.meta.database_name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Merged".into());
            let gid = {
                let mut root = self.db.root_mut();
                let mut g = root.add_group();
                g.name = label;
                g.id()
            };
            let mut s = ImportSummary::default();
            self.copy_tree(other.root(), gid, other.recycle_bin().map(|b| b.id()), &mut s);
            self.save()?;
            return Ok(MergeSummary {
                added: s.entries,
                warnings: vec!["This file isn't a copy of your vault, so everything in it came in as new entries in its own folder.".into()],
                ..Default::default()
            });
        }
        let log = self.db.merge(&other).map_err(|e| anyhow!("The merge stopped: {e}"))?;
        // The merge's own warnings are technical (timestamps, IDs); they aren't shown.
        let mut m = MergeSummary::default();
        for ev in &log.events {
            if let keepass::db::merge::MergeEventTarget::Entry(_) = ev.target {
                match ev.event_type {
                    keepass::db::merge::MergeEventType::Created => m.added += 1,
                    keepass::db::merge::MergeEventType::Updated => m.updated += 1,
                    keepass::db::merge::MergeEventType::LocationUpdated => m.moved += 1,
                    keepass::db::merge::MergeEventType::Deleted => m.deleted += 1,
                    _ => {}
                }
            }
        }
        self.ensure_recycle_bin();
        self.save()?;
        Ok(m)
    }

    /// Every entry outside the recycle bin, for the health check and auto-type.
    pub(crate) fn live_entries(&self) -> impl Iterator<Item = keepass::db::EntryRef<'_>> + '_ {
        self.db.iter_all_entries().filter(|e| !self.in_bin(e.parent().id()))
    }

    pub(crate) fn summary_of(&self, e: &keepass::db::EntryRef<'_>) -> EntrySummary {
        self.summary(e)
    }

    /// Copies a whole folder tree from another database into this one.
    fn copy_tree(&mut self, src: keepass::db::GroupRef<'_>, dst: GroupId, skip: Option<GroupId>, s: &mut ImportSummary) {
        for e in src.entries() {
            let Some(mut g) = self.db.group_mut(dst) else { return };
            let mut n = g.add_entry();
            n.fields = e.fields.clone();
            n.tags = e.tags.clone();
            n.times = e.times.clone();
            n.autotype = e.autotype.clone();
            n.override_url = e.override_url.clone();
            for (name, a) in e.attachments_named() {
                n.add_attachment(name.to_string(), a.data.clone());
            }
            if let Some(ci) = e.custom_icon() {
                n.set_icon_custom_new(ci.data.clone());
            }
            s.entries += 1;
        }
        for k in src.groups() {
            if Some(k.id()) == skip {
                continue;
            }
            let Some(mut g) = self.db.group_mut(dst) else { return };
            let mut ng = g.add_group();
            ng.name = k.name.clone();
            ng.notes = k.notes.clone();
            ng.tags = k.tags.clone();
            ng.enable_autotype = k.enable_autotype;
            ng.default_autotype_sequence = k.default_autotype_sequence.clone();
            let nid = ng.id();
            s.groups += 1;
            self.copy_tree(k, nid, skip, s);
        }
    }

    /// Brings in a KeePass file (KDBX 3 or 4, or a KDB from KeePass 1) as a new folder.
    pub fn import_kdbx(&mut self, bytes: &[u8], password: &str, keyfile: Option<&[u8]>, name: &str) -> Result<ImportSummary> {
        let other = Database::parse(bytes, user_key(password, keyfile)?).map_err(|e| match e {
            keepass::db::DatabaseOpenError::Key(_) => anyhow!("The password or key file isn't right for this file."),
            other => anyhow!("This file couldn't be opened as a KeePass database: {other}"),
        })?;
        let label = if name.trim().is_empty() { "Imported".to_string() } else { name.trim().to_string() };
        let gid = {
            let mut root = self.db.root_mut();
            let mut g = root.add_group();
            g.name = label;
            g.id()
        };
        let mut s = ImportSummary { group_id: gid.uuid().to_string(), ..Default::default() };
        let skip = other.recycle_bin().map(|b| b.id());
        self.copy_tree(other.root(), gid, skip, &mut s);
        self.save()?;
        Ok(s)
    }

    /// Saves a copy as a normal KeePass file that KeePassXC and KeePass open with this password.
    pub fn export_kdbx(&self, dest: &Path, password: &str, keyfile: Option<&[u8]>) -> Result<()> {
        if password.chars().count() < 8 && keyfile.is_none() {
            bail!("Use a password of at least 8 characters for the exported file.");
        }
        let mut copy = self.db.clone();
        copy.config.kdf_config = export_kdf();
        copy.meta.generator = Some("Sompio".into());
        let mut buf = Vec::new();
        copy.save(&mut buf, user_key(password, keyfile)?).map_err(|e| anyhow!("Couldn't write the file: {e}"))?;
        std::fs::write(dest, buf)?;
        Ok(())
    }

    /// Adds entries made elsewhere (CSV imports), in one new folder with sub-folders by path.
    pub(crate) fn add_imported(&mut self, label: &str, rows: Vec<(Vec<String>, EntryInput)>) -> Result<ImportSummary> {
        let top = {
            let mut root = self.db.root_mut();
            let mut g = root.add_group();
            g.name = label.to_string();
            g.id()
        };
        let mut s = ImportSummary { group_id: top.uuid().to_string(), ..Default::default() };
        let mut made: HashMap<Vec<String>, GroupId> = HashMap::new();
        for (path, mut input) in rows {
            let mut parent = top;
            for depth in 1..=path.len() {
                let key = path[..depth].to_vec();
                parent = match made.get(&key) {
                    Some(g) => *g,
                    None => {
                        let mut p = self.db.group_mut(parent).ok_or_else(|| anyhow!("folder vanished"))?;
                        let mut g = p.add_group();
                        g.name = path[depth - 1].clone();
                        let id = g.id();
                        made.insert(key, id);
                        s.groups += 1;
                        id
                    }
                };
            }
            input.group_id = None;
            if input.title.trim().is_empty() {
                input.title = if !input.url.is_empty() { input.url.clone() } else { "Untitled".into() };
            }
            let mut g = self.db.group_mut(parent).ok_or_else(|| anyhow!("folder vanished"))?;
            let mut e = g.add_entry();
            match Self::apply(&mut e, &input) {
                Ok(()) => s.entries += 1,
                Err(err) => {
                    let title = input.title.clone();
                    e.remove();
                    s.warnings.push(format!("{title}: {err}"));
                }
            }
        }
        self.save()?;
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(title: &str, user: &str, pw: &str) -> EntryInput {
        EntryInput { title: title.into(), username: user.into(), password: Some(pw.into()), url: "https://example.org/login".into(), ..Default::default() }
    }

    #[test]
    fn create_edit_history_bin_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.kdbx");
        let key = [7u8; 32];
        let id;
        {
            let mut v = Vault::open(&path, &key).unwrap();
            let folder = v.add_group(None, "Games").unwrap();
            let mut i = input("Battle.net", "sam", "first-password");
            i.group_id = Some(folder.clone());
            i.custom = vec![FieldInput { key: "PIN".into(), value: Some("4321".into()), protected: true }];
            i.otp = Some("JBSWY3DPEHPK3PXP".into());
            id = v.add_entry(&i).unwrap();
            let mut j = input("Battle.net", "sam", "second-password");
            j.custom = vec![FieldInput { key: "PIN".into(), value: None, protected: true }];
            v.update_entry(&id, &j).unwrap();
            let d = v.entry(&id).unwrap();
            assert_eq!(d.history.len(), 1);
            assert!(d.history[0].password_changed);
            assert_eq!(d.custom[0].value, None, "hidden until revealed");
            assert_eq!(&*v.reveal(&id, "PIN").unwrap(), "4321", "kept without being re-entered");
            assert!(d.summary.has_totp);
            assert_eq!(v.totp(&id).unwrap().unwrap().code.len(), 6);
            assert_eq!(v.entries(Some(&folder), "").unwrap().len(), 1);
            assert_eq!(v.entries(None, "battle skip").unwrap().len(), 1);
            assert!(v.add_entry(&EntryInput { title: " ".into(), ..Default::default() }).is_err());
        }
        let raw = std::fs::read(&path).unwrap();
        assert!(!raw.windows(15).any(|w| w == b"second-password"));
        assert!(Vault::open(&path, &[8u8; 32]).is_err());
        let mut v = Vault::open(&path, &key).unwrap();
        assert_eq!(&*v.reveal(&id, fields::PASSWORD).unwrap(), "second-password");
        v.restore_version(&id, 0).unwrap();
        assert_eq!(&*v.reveal(&id, fields::PASSWORD).unwrap(), "first-password");
        assert!(!v.delete_entry(&id).unwrap(), "to the bin first");
        assert!(v.entries(None, "").unwrap().is_empty());
        v.restore_entry(&id).unwrap();
        assert_eq!(v.entries(None, "").unwrap()[0].group_path, "Games");
        v.delete_entry(&id).unwrap();
        assert_eq!(v.empty_recycle_bin().unwrap(), 1);
        assert!(v.entry(&id).is_err());
    }

    #[test]
    fn export_opens_with_its_password_and_imports_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = Vault::open(&dir.path().join("vault.kdbx"), &[1u8; 32]).unwrap();
        let g = v.add_group(None, "Mail").unwrap();
        let mut i = input("Proton", "me@proton.me", "hunter2-but-longer");
        i.group_id = Some(g);
        v.add_entry(&i).unwrap();
        let out = dir.path().join("export.kdbx");
        assert!(v.export_kdbx(&out, "short", None).is_err());
        v.export_kdbx(&out, "correct horse battery", None).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        let other = Database::parse(&bytes, DatabaseKey::new().with_password("correct horse battery")).unwrap();
        assert_eq!(other.iter_all_entries().count(), 1);
        assert!(v.import_kdbx(&bytes, "wrong", None, "x").is_err());
        let s = v.import_kdbx(&bytes, "correct horse battery", None, "From laptop").unwrap();
        assert_eq!(s.skipped_attachments, 0);
        assert_eq!((s.entries, s.groups), (1, 1), "the bin isn't copied");
        assert_eq!(v.entries(None, "proton").unwrap().len(), 2);
        assert!(v.entries(None, "proton").unwrap().iter().any(|e| e.group_path == "From laptop / Mail"));
    }
}

#[cfg(test)]
mod round2 {
    use super::*;

    fn vault(dir: &Path) -> Vault {
        Vault::open(&dir.join("vault.kdbx"), &[6u8; 32]).unwrap()
    }

    #[test]
    fn icons_are_stored_shared_and_cleaned_up() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = vault(dir.path());
        let a = v.add_entry(&EntryInput { title: "Battle.net".into(), url: "battle.net".into(), ..Default::default() }).unwrap();
        let b = v.add_entry(&EntryInput { title: "Battle.net 2".into(), url: "battle.net".into(), ..Default::default() }).unwrap();
        assert_eq!(v.entries_without_icon().len(), 2);
        let png = vec![137, 80, 78, 71, 1, 2, 3];
        let ia = v.set_icon(&a, Some(png.clone())).unwrap().unwrap();
        let ib = v.set_icon(&b, Some(png.clone())).unwrap().unwrap();
        assert_eq!(ia, ib, "same picture is stored once");
        assert_eq!(v.icons().len(), 1);
        let c = v.clone_entry(&a).unwrap();
        assert_eq!(v.entry(&c).unwrap().summary.icon.as_deref(), Some(ia.as_str()));
        // Survives saving and opening again
        drop(v);
        let mut v = vault(dir.path());
        assert_eq!(v.entry(&a).unwrap().summary.icon.as_deref(), Some(ia.as_str()));
        v.set_icon(&a, None).unwrap();
        v.set_icon(&b, Some(vec![137, 80, 78, 71, 9])).unwrap();
        assert_eq!(v.icons().len(), 2, "the clone still uses the first one");
        v.delete_entry(&c).unwrap();
        v.delete_entry(&c).unwrap();
        v.set_icon(&b, None).unwrap();
        assert_eq!(v.icons().len(), 0, "unused icons are dropped");
    }

    #[test]
    fn clone_copies_everything_but_history() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = vault(dir.path());
        let id = v.add_entry(&EntryInput { title: "Mail".into(), username: "me".into(), password: Some("pw-123456".into()), ..Default::default() }).unwrap();
        v.add_attachment(&id, "codes.txt", b"123".to_vec()).unwrap();
        let c = v.clone_entry(&id).unwrap();
        assert_ne!(c, id);
        let d = v.entry(&c).unwrap();
        assert_eq!(d.summary.title, "Mail - Clone");
        assert_eq!(d.summary.username, "me");
        assert_eq!(v.reveal(&c, "Password").unwrap().as_str(), "pw-123456");
        assert_eq!(d.attachments.len(), 1);
        assert!(d.history.is_empty());
    }

    #[test]
    fn attachments_survive_save_export_and_import() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = vault(dir.path());
        let id = v.add_entry(&EntryInput { title: "Passport".into(), ..Default::default() }).unwrap();
        v.add_attachment(&id, "scan.pdf", b"%PDF-1.7 fake".to_vec()).unwrap();
        v.add_attachment(&id, "notes.txt", b"hello".to_vec()).unwrap();
        assert!(v.add_attachment(&id, "big.bin", vec![0; MAX_ATTACHMENT + 1]).is_err());
        let d = v.entry(&id).unwrap();
        assert_eq!(d.attachments.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["notes.txt", "scan.pdf"]);
        drop(v);
        let mut v = vault(dir.path());
        assert_eq!(&*v.attachment(&id, "scan.pdf").unwrap(), b"%PDF-1.7 fake");
        v.remove_attachment(&id, "notes.txt").unwrap();
        assert_eq!(v.entry(&id).unwrap().attachments.len(), 1);
        let out = dir.path().join("x.kdbx");
        v.export_kdbx(&out, "export-password", None).unwrap();
        let mut w = Vault::open(&dir.path().join("w.kdbx"), &[1u8; 32]).unwrap();
        w.import_kdbx(&std::fs::read(&out).unwrap(), "export-password", None, "x").unwrap();
        let wid = w.entries(None, "passport").unwrap()[0].id.clone();
        assert_eq!(&*w.attachment(&wid, "scan.pdf").unwrap(), b"%PDF-1.7 fake");
    }

    #[test]
    fn merge_brings_back_changes_from_a_copy() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = vault(dir.path());
        let a = v.add_entry(&EntryInput { title: "Mail".into(), password: Some("old".into()), ..Default::default() }).unwrap();
        let out = dir.path().join("copy.kdbx");
        v.export_kdbx(&out, "copy-password", None).unwrap();
        // Edit the copy elsewhere: change Mail, add Bank
        let mut copy = Database::parse(&std::fs::read(&out).unwrap(), user_key("copy-password", None).unwrap()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        {
            let mut e = copy.entry_mut(parse_entry_id(&a).unwrap()).unwrap();
            e.edit_tracking(|t| t.set_protected(fields::PASSWORD, "new-from-laptop"));
        }
        {
            let mut root = copy.root_mut();
            let mut n = root.add_entry();
            n.set_unprotected(fields::TITLE, "Bank");
        }
        let mut buf = Vec::new();
        copy.save(&mut buf, user_key("copy-password", None).unwrap()).unwrap();
        let m = v.merge_kdbx(&buf, "copy-password", None).unwrap();
        assert_eq!((m.added, m.updated), (1, 1));
        assert_eq!(&*v.reveal(&a, fields::PASSWORD).unwrap(), "new-from-laptop");
        assert_eq!(v.entry(&a).unwrap().history.len(), 1, "the old version is kept");
        // Merging again changes nothing
        let again = v.merge_kdbx(&buf, "copy-password", None).unwrap();
        assert_eq!((again.added, again.updated), (0, 0));
        // A foreign file comes in as its own folder
        let mut other = Database::new();
        other.meta.database_name = Some("Other".into());
        other.root_mut().add_entry().set_unprotected(fields::TITLE, "Foreign");
        let mut b2 = Vec::new();
        other.save(&mut b2, user_key("pw-pw-pw-pw", None).unwrap()).unwrap();
        let f = v.merge_kdbx(&b2, "pw-pw-pw-pw", None).unwrap();
        assert_eq!(f.added, 1);
        assert_eq!(v.entries(None, "foreign").unwrap()[0].group_path, "Other");
    }
}
