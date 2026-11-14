//! Scans folders on this computer for files that hold personal information.
//!
//! Only folders the user picks are read, and only when they start the scan. Text-like files and
//! Word documents are read for their text; every file's name is also checked (a photo called
//! "passport.jpg" is worth knowing about). Two more checks are off unless the person turns them on
//! for that scan: reading the text in pictures (screenshots, scans) and reading the place a photo
//! was taken from its GPS data. Neither keeps a copy of the picture.
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
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "heic", "heif", "webp", "tif", "tiff", "bmp"];
const MAX_IMAGE_BYTES: u64 = 40 * 1024 * 1024;

/// The extra checks for one folder scan. Both are off unless the person turns them on.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct PcScanOptions {
    /// Read the text in screenshots and scanned documents
    #[serde(default)]
    pub image_text: bool,
    /// Read where photos were taken from their GPS data
    #[serde(default)]
    pub photo_places: bool,
}

/// Reads the text in a picture. The desktop app passes Windows' own offline text recognition.
pub type ImageReader<'a> = &'a dyn Fn(&Path) -> Option<String>;

pub fn is_image(ext: &str) -> bool {
    IMAGE_EXTS.contains(&ext)
}

/// Where a photo was taken, from the GPS data cameras and phones write into it.
pub fn photo_location(path: &Path) -> Option<(f64, f64)> {
    use exif::{In, Tag, Value};
    let file = std::fs::File::open(path).ok()?;
    let data = exif::Reader::new().read_from_container(&mut std::io::BufReader::new(file)).ok()?;
    let degrees = |tag: Tag| -> Option<f64> {
        match &data.get_field(tag, In::PRIMARY)?.value {
            Value::Rational(v) if v.len() == 3 => Some(v[0].to_f64() + v[1].to_f64() / 60.0 + v[2].to_f64() / 3600.0),
            _ => None,
        }
    };
    let side = |tag: Tag| data.get_field(tag, In::PRIMARY).map(|f| f.display_value().to_string()).unwrap_or_default();
    let mut lat = degrees(Tag::GPSLatitude)?;
    let mut lon = degrees(Tag::GPSLongitude)?;
    if side(Tag::GPSLatitudeRef).starts_with('S') {
        lat = -lat;
    }
    if side(Tag::GPSLongitudeRef).starts_with('W') {
        lon = -lon;
    }
    let ok = lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0 && (lat != 0.0 || lon != 0.0);
    ok.then_some((lat, lon))
}

/// The line a photo's place is written as, so the scanner can flag it. Five decimals is about a metre.
pub(crate) fn location_line(lat: f64, lon: f64) -> String {
    format!("[Photo location: {lat:.5}, {lon:.5}]")
}
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

/// Scans the given folders with only the basic checks (file names, text files, Word documents).
pub fn scan_folders(store: &mut Store, folders: &[PathBuf], progress: &mut dyn FnMut(ImportProgress)) -> Result<ImportSummary> {
    scan_folders_with(store, folders, PcScanOptions::default(), None, progress)
}

/// Scans the given folders, replacing the previous PC scan. The picture checks run only when
/// `opts` turns them on; text in pictures also needs a `reader`.
pub fn scan_folders_with(
    store: &mut Store,
    folders: &[PathBuf],
    opts: PcScanOptions,
    reader: Option<ImageReader>,
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
            let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            let picture = is_image(&ext);
            let slow = picture && opts.image_text && reader.is_some();
            if (from + i) % 500 == 0 || (slow && (from + i) % 10 == 0) {
                let phase = if opts.image_text && reader.is_some() { "Reading files and pictures" } else { "Reading files" };
                progress(ImportProgress { phase, done: from + i, total });
            }
            let Ok(meta) = std::fs::metadata(path) else { continue };
            let ts = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let mut content = rel.clone();
            let mut body = Vec::new();
            if let Some(text) = read_text(path, &ext, meta.len()) {
                body.push(text);
            }
            if picture && meta.len() <= MAX_IMAGE_BYTES {
                if opts.photo_places {
                    if let Some((lat, lon)) = photo_location(path) {
                        body.push(location_line(lat, lon));
                    }
                }
                if let (true, Some(read)) = (opts.image_text, reader) {
                    if let Some(text) = read(path) {
                        body.push(text);
                    }
                }
            }
            if !body.is_empty() {
                content.push_str("\n\n");
                content.push_str(&body.join("\n\n"));
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
