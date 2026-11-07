//! Scans folders on this computer for files that hold personal information.
//!
//! Only folders the user picks are read. Text-like files and Word documents are read
//! for their text; every file's name is also checked (a photo called "passport.jpg"
//! is worth knowing about even though Veil can't read the picture).
//! Each file becomes one "message" whose first line is its path inside the folder.

use crate::import::{after_import, ImportProgress, ImportSummary, Writer};
use crate::Store;
use anyhow::{anyhow, Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};

const TEXT_EXTS: &[&str] = &[
    "txt", "md", "csv", "tsv", "json", "log", "ini", "cfg", "conf", "xml", "html", "htm", "yaml", "yml", "env", "rtf",
    "eml", "vcf", "sql", "toml", "properties",
];
const MAX_TEXT_BYTES: u64 = 5 * 1024 * 1024;
const MAX_CHARS: usize = 1_000_000;
const MAX_FILES: usize = 200_000;
const MAX_DEPTH: usize = 16;

/// Folders that are never personal documents, or are far too big to be worth it.
fn skip_dir(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with('.')
        || n.starts_with('$')
        || matches!(
            n.as_str(),
            "node_modules" | "appdata" | "windows" | "program files" | "program files (x86)" | "programdata"
                | "target" | "venv" | "__pycache__" | "site-packages" | "steamapps" | "system volume information"
        )
}

fn read_docx(path: &Path) -> Result<String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    let mut xml = String::new();
    zip.by_name("word/document.xml")?.read_to_string(&mut xml)?;
    // Paragraph ends become new lines, then every tag is dropped.
    let xml = xml.replace("</w:p>", "\n").replace("<w:tab/>", "\t");
    let mut out = String::with_capacity(xml.len() / 3);
    let mut in_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    Ok(out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'"))
}

fn read_text(path: &Path, ext: &str, size: u64) -> Option<String> {
    if ext == "docx" {
        return read_docx(path).ok();
    }
    if !TEXT_EXTS.contains(&ext) || size > MAX_TEXT_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if bytes.iter().take(4096).filter(|b| **b == 0).count() > 8 {
        return None; // binary after all
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn truncate(mut s: String) -> String {
    if s.len() > MAX_CHARS {
        let mut cut = MAX_CHARS;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
    }
    s
}

struct Walker<'p> {
    files: Vec<(PathBuf, String)>, // full path, path relative to its root
    progress: &'p mut dyn FnMut(ImportProgress),
}

impl Walker<'_> {
    fn walk(&mut self, root: &Path, dir: &Path, depth: usize) {
        if depth > MAX_DEPTH || self.files.len() >= MAX_FILES {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let path = entry.path();
            if ft.is_dir() {
                if !skip_dir(&name) {
                    self.walk(root, &path, depth + 1);
                }
            } else if ft.is_file() && !name.starts_with("~$") {
                let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                self.files.push((path, rel));
                if self.files.len() % 2000 == 0 {
                    (self.progress)(ImportProgress { phase: "Finding files", done: self.files.len(), total: 0 });
                }
            }
        }
    }
}

/// Scans the given folders, replacing the previous PC scan.
pub fn scan_folders(
    store: &mut Store,
    folders: &[PathBuf],
    progress: &mut dyn FnMut(ImportProgress),
) -> Result<ImportSummary> {
    if folders.is_empty() {
        return Err(anyhow!("Choose at least one folder to scan."));
    }
    let mut roots = Vec::new();
    for f in folders {
        let root = f.canonicalize().with_context(|| format!("can't open {}", f.display()))?;
        if !root.is_dir() {
            return Err(anyhow!("{} is not a folder", f.display()));
        }
        roots.push(root);
    }

    let mut walker = Walker { files: Vec::new(), progress };
    let mut per_root = Vec::new();
    for root in &roots {
        let before = walker.files.len();
        walker.walk(root, root, 0);
        per_root.push((root.clone(), before, walker.files.len()));
    }
    let files = std::mem::take(&mut walker.files);
    let progress = walker.progress;

    let mut summary = ImportSummary::default();
    if files.len() >= MAX_FILES {
        summary.warnings.push(format!("Stopped after {MAX_FILES} files. Choose smaller folders to scan the rest."));
    }
    let mut w = Writer::new(store, "pc")?;
    let total = files.len();
    for (root, from, to) in per_root {
        let root_name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| root.display().to_string());
        let root_str = display_path(&root);
        let channel = w.channel(&root_str, &root_name, "folder", None, Some(&root_str))?;
        for (i, (path, rel)) in files[from..to].iter().enumerate() {
            if (from + i) % 500 == 0 {
                progress(ImportProgress { phase: "Reading files", done: from + i, total });
            }
            let Ok(meta) = std::fs::metadata(path) else { continue };
            let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            let ts = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let mut content = rel.clone();
            if let Some(text) = read_text(path, &ext, meta.len()) {
                content.push_str("\n\n");
                content.push_str(&text);
            }
            let full = display_path(path);
            w.message(&full, channel, ts, &truncate(content), &full, None)?;
        }
    }
    summary.platform = "pc".to_string();
    summary.messages = w.messages;
    summary.channels = w.channel_count();
    let origin = roots.iter().map(|r| display_path(r)).collect::<Vec<_>>().join("\n");
    w.finish(&origin)?;
    progress(ImportProgress { phase: "Building search index", done: total, total });
    after_import(store)?;
    Ok(summary)
}

/// Windows' canonical paths start with \\?\ — hide that from people.
fn display_path(p: &Path) -> String {
    let s = p.display().to_string();
    s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
}

/// Words in a file's name that suggest an identity document, money matters or secrets.
/// Matched against the first line of a PC-scan "message" (the file's path).
pub(crate) const SENSITIVE_NAME_WORDS: &[&str] = &[
    "passport", "paspoort", "pasport", "pasaport", "identity card", "id card", "idcard", "id-card", "id kaart",
    "id-kaart", "identiteitskaart", "rijbewijs", "drivers license", "driver license", "driver's license",
    "driving licence", "permis de conducere", "buletin", "carte de identitate", "residence permit", "verblijfsvergunning", "bsn",
    "social security", "ssn", "cnp", "payslip", "pay slip", "loonstrook", "salarisstrook", "fluturas", "salary",
    "tax return", "belastingaangifte", "aangifte", "jaaropgaaf", "bank statement", "bankafschrift", "extras de cont",
    "passwords", "password", "wachtwoord", "wachtwoorden", "parole", "credentials", "recovery code", "recovery codes",
    "backup codes", "seed phrase", "recovery phrase", "private key", "medical", "medisch", "diagnose", "diagnosis",
    "contract", "lease", "huurcontract", "cv", "resume", "curriculum vitae", "birth certificate", "geboorteakte", "certificat de nastere",
];

/// File names and extensions that are keys or credential stores in themselves.
pub(crate) fn is_key_file(name_lower: &str) -> bool {
    let base = name_lower.rsplit('/').next().unwrap_or(name_lower);
    matches!(base, "id_rsa" | "id_ed25519" | "id_ecdsa" | "id_dsa" | ".env" | "credentials" | "wallet.dat")
        || [".pem", ".key", ".ppk", ".pfx", ".p12", ".kdbx", ".ovpn", ".asc"].iter().any(|e| base.ends_with(e))
}

/// Moves a scanned file to the Recycle Bin (Trash on other systems).
pub fn move_to_recycle_bin(path: &str) -> Result<()> {
    trash::delete(path).map_err(|e| anyhow!("Couldn't move it to the Recycle Bin: {e}"))
}
