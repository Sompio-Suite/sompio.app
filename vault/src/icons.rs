//! Website icons for entries, like KeePassXC's "Download favicon". This part never touches the
//! network: it finds icon links in a page, and turns whatever comes back into a small PNG. The
//! downloading is in the shell, and only ever goes to the entry's own website (or an address the
//! person typed), never to an icon service in between.

use anyhow::{anyhow, bail, Result};
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageFormat, RgbaImage};

/// Icons are stored at this size (KeePassXC uses 64 too, and scales them down).
pub const SIZE: u32 = 64;
/// Downloads bigger than this are ignored: real favicons are a few kilobytes.
pub const MAX_ICON_BYTES: u64 = 1024 * 1024;
pub const MAX_PAGE_BYTES: u64 = 2 * 1024 * 1024;

/// "zoom.us", "https://eu.zoom.us/signin" or "account.proton.me" -> "https://eu.zoom.us/"
pub fn site_root(address: &str) -> Result<String> {
    let a = address.trim();
    if a.is_empty() {
        bail!("This entry has no website. Add one, or enter the address to get the icon from.");
    }
    let with_scheme = if a.contains("://") { a.to_string() } else { format!("https://{a}") };
    let (scheme, rest) = with_scheme.split_once("://").ok_or_else(|| anyhow!("That isn't a web address."))?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && scheme != "http" {
        bail!("Icons can only come from a web address (https://…).");
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").rsplit('@').next().unwrap_or("");
    if host.is_empty() || !host.contains('.') && !host.starts_with('[') {
        bail!("\"{a}\" doesn't look like a website address.");
    }
    Ok(format!("{scheme}://{}/", host.to_ascii_lowercase()))
}

/// The main domain, for a second try: "https://account.proton.me/" -> "https://proton.me/"
pub fn parent_root(root: &str) -> Option<String> {
    let (scheme, rest) = root.split_once("://")?;
    let host = rest.trim_end_matches('/');
    let (host_only, port) = match host.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => (h, format!(":{p}")),
        _ => (host, String::new()),
    };
    if host_only.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    let labels: Vec<&str> = host_only.split('.').collect();
    // Keep two labels, or three for "co.uk"-style endings
    let keep = if labels.len() >= 3 && labels[labels.len() - 2].len() <= 3 && labels[labels.len() - 1].len() == 2 { 3 } else { 2 };
    if labels.len() <= keep {
        return None;
    }
    Some(format!("{scheme}://{}{port}/", labels[labels.len() - keep..].join(".")))
}

/// Icon addresses in a page, best first (bigger declared sizes and apple-touch-icons first), then
/// the classic /favicon.ico.
pub fn candidates(page_url: &str, html: &str) -> Vec<String> {
    let mut found: Vec<(u32, String)> = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut at = 0;
    while let Some(i) = lower[at..].find("<link") {
        let start = at + i;
        let end = lower[start..].find('>').map(|e| start + e).unwrap_or(lower.len());
        let tag = &html[start..end];
        let tag_l = &lower[start..end];
        at = end;
        let Some(rel) = attr(tag, tag_l, "rel") else { continue };
        let rel = rel.to_ascii_lowercase();
        let is_icon = rel.split_whitespace().any(|r| r == "icon" || r == "apple-touch-icon" || r == "apple-touch-icon-precomposed");
        if !is_icon || rel.contains("mask-icon") {
            continue;
        }
        let Some(href) = attr(tag, tag_l, "href") else { continue };
        if href.trim().is_empty() || href.to_ascii_lowercase().ends_with(".svg") || href.starts_with("data:image/svg") {
            continue;
        }
        let size = attr(tag, tag_l, "sizes")
            .and_then(|s| s.split_whitespace().filter_map(|x| x.split(['x', 'X']).next()?.parse::<u32>().ok()).max())
            .unwrap_or(if rel.contains("apple-touch") { 180 } else { 32 });
        // Prefer icons near 64 px or bigger; huge ones are fine too (scaled down)
        let score = if size >= 48 { 1000 + size.min(512) } else { size };
        if let Some(abs) = resolve(page_url, &decode_entities(href.trim())) {
            found.push((score, abs));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out: Vec<String> = Vec::new();
    for (_, u) in found {
        if !out.contains(&u) {
            out.push(u);
        }
    }
    if let Ok(root) = site_root(page_url) {
        for extra in ["favicon.ico", "apple-touch-icon.png", "favicon.png"] {
            let u = format!("{root}{extra}");
            if !out.contains(&u) {
                out.push(u);
            }
        }
    }
    out
}

fn attr<'a>(tag: &'a str, tag_l: &str, name: &str) -> Option<&'a str> {
    // tag_l is tag in lower case: same bytes, same positions
    let b = tag_l.as_bytes();
    let mut from = 0;
    while let Some(i) = tag_l[from..].find(name) {
        let p = from + i;
        from = p + name.len();
        if p > 0 && !b[p - 1].is_ascii_whitespace() {
            continue;
        }
        let mut q = p + name.len();
        while q < b.len() && b[q].is_ascii_whitespace() {
            q += 1;
        }
        if q >= b.len() || b[q] != b'=' {
            continue;
        }
        q += 1;
        while q < b.len() && b[q].is_ascii_whitespace() {
            q += 1;
        }
        if q >= b.len() {
            return None;
        }
        let v = &tag[q..];
        return Some(match b[q] {
            b'"' | b'\'' => v[1..].split(b[q] as char).next().unwrap_or(""),
            _ => v.split(char::is_whitespace).next().unwrap_or("").trim_end_matches('/'),
        });
    }
    None
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&").replace("&#38;", "&").replace("&quot;", "\"")
}

/// A link in a page, made absolute.
fn resolve(page: &str, href: &str) -> Option<String> {
    if href.starts_with("data:") {
        return None;
    }
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }
    let (scheme, rest) = page.split_once("://")?;
    if let Some(r) = href.strip_prefix("//") {
        return Some(format!("{scheme}://{r}"));
    }
    let host_end = rest.find('/').unwrap_or(rest.len());
    let origin = format!("{scheme}://{}", &rest[..host_end]);
    if href.starts_with('/') {
        return Some(format!("{origin}{href}"));
    }
    let path = &rest[host_end..];
    let path = path.split(['?', '#']).next().unwrap_or("/");
    let dir = match path.rfind('/') {
        Some(i) => &path[..=i],
        None => "/",
    };
    Some(format!("{origin}{dir}{href}"))
}

/// Any icon file (ICO, PNG, JPEG, GIF, WebP, BMP) -> a 64x64 PNG, centred on a transparent square.
/// From an ICO, the largest picture in it is used.
pub fn to_png(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() < 16 {
        bail!("That isn't a picture.");
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(256)]).to_ascii_lowercase();
    if head.contains("<svg") || head.contains("<?xml") {
        bail!("That icon is an SVG drawing, which can't be stored. Try a PNG or ICO file.");
    }
    if head.contains("<html") || head.contains("<!doctype") {
        bail!("That address gave a web page, not a picture.");
    }
    let format = image::guess_format(bytes).map_err(|_| anyhow!("That isn't a picture Sompio can read (PNG, ICO, JPEG, GIF, WebP or BMP)."))?;
    let img: DynamicImage = image::load_from_memory_with_format(bytes, format).map_err(|e| anyhow!("Couldn't read that picture: {e}"))?;
    let (w, h) = img.dimensions();
    if w < 8 || h < 8 {
        bail!("That icon is too small to use.");
    }
    let big = w.max(h);
    // Bigger pictures shrink to fit; small ones grow by a whole step (16 -> 64, 24 -> 48)
    let scale = if big >= SIZE { SIZE as f32 / big as f32 } else { (SIZE / big) as f32 };
    let (nw, nh) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
    // Small icons are scaled up by whole steps, so they stay crisp
    let filter = if nw > w { FilterType::Nearest } else { FilterType::Lanczos3 };
    let small = img.resize_exact(nw, nh, filter).to_rgba8();
    let mut canvas = RgbaImage::new(SIZE, SIZE);
    image::imageops::overlay(&mut canvas, &small, ((SIZE - nw) / 2) as i64, ((SIZE - nh) / 2) as i64);
    if canvas.pixels().all(|p| p.0[3] == 0) {
        bail!("That icon is empty.");
    }
    let mut out = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas).write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}

pub fn data_url(png: &[u8]) -> String {
    use base64::Engine;
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots() {
        assert_eq!(site_root("zoom.us").unwrap(), "https://zoom.us/");
        assert_eq!(site_root("https://EU.Zoom.us/signin?x=1").unwrap(), "https://eu.zoom.us/");
        assert_eq!(site_root("http://192.168.1.1:8080/admin").unwrap(), "http://192.168.1.1:8080/");
        assert!(site_root("ftp://x.org").is_err());
        assert!(site_root("Home Wi-Fi").is_err());
        assert_eq!(parent_root("https://account.proton.me/").as_deref(), Some("https://proton.me/"));
        assert_eq!(parent_root("https://www.bbc.co.uk/").as_deref(), Some("https://bbc.co.uk/"));
        assert_eq!(parent_root("https://proton.me/"), None);
        assert_eq!(parent_root("http://192.168.1.1/"), None);
    }

    #[test]
    fn finds_icons_in_a_page() {
        let html = r#"<html><head>
            <link rel="stylesheet" href="/a.css">
            <link rel="icon" type="image/png" sizes="16x16" href="/static/fav-16.png">
            <LINK REL="apple-touch-icon" HREF='https://cdn.example.org/touch.png'>
            <link rel="icon" href="logo.svg">
            <link href="//cdn.example.org/icon-192.png" rel="icon" sizes="192x192" />
            <link rel="shortcut icon" href="img/fav.ico?v=2&amp;x=1">
        </head></html>"#;
        let c = candidates("https://www.example.org/login/page", html);
        assert_eq!(c[0], "https://cdn.example.org/icon-192.png");
        assert_eq!(c[1], "https://cdn.example.org/touch.png");
        assert!(c.contains(&"https://www.example.org/login/img/fav.ico?v=2&x=1".to_string()));
        assert!(c.contains(&"https://www.example.org/static/fav-16.png".to_string()));
        assert!(!c.iter().any(|u| u.ends_with(".svg")));
        assert_eq!(c.last().unwrap(), "https://www.example.org/favicon.png");
        assert!(c.contains(&"https://www.example.org/favicon.ico".to_string()));
    }

    fn sample(w: u32, h: u32, fmt: ImageFormat) -> Vec<u8> {
        let img = RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x * 7) as u8, (y * 5) as u8, 120, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, fmt).unwrap();
        out.into_inner()
    }

    #[test]
    fn converts_to_a_64px_png() {
        for (w, h, f) in [(16, 16, ImageFormat::Ico), (32, 32, ImageFormat::Png), (180, 180, ImageFormat::Png), (300, 150, ImageFormat::Jpeg), (48, 48, ImageFormat::Bmp)] {
            let png = to_png(&sample(w, h, f)).unwrap();
            let back = image::load_from_memory(&png).unwrap();
            assert_eq!(back.dimensions(), (SIZE, SIZE), "{w}x{h} {f:?}");
        }
        assert!(to_png(b"<svg xmlns='http://www.w3.org/2000/svg'></svg>").is_err());
        assert!(to_png(b"<!DOCTYPE html><html><body>nope</body></html>").is_err());
        assert!(to_png(&[0u8; 40]).is_err());
    }
}
