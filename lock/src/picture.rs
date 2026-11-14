//! The key picture: a night sky with the polar sun in Sompio's white and green, and a 256-bit
//! secret hidden inside.
//!
//! The secret is stored twice so ordinary file handling doesn't lose it:
//! * in a private PNG chunk ("smKy"), and
//! * three times in the lowest bit of the blue channel along three rows of pixels.
//! Either copy is enough. Apps that shrink or convert photos (most chat apps) destroy both,
//! which is why people are told to move it as a file.
//!
//! The stars are random for every picture, so each person's picture looks a little different.
//! They come from a separate random seed, never from the secret.

use anyhow::{anyhow, bail, Result};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop};

const W: u32 = 1080;
const H: u32 = 1080;
const MAGIC: &[u8; 4] = b"SMPK";
const VERSION: u8 = 1;
const PAYLOAD_LEN: usize = 4 + 1 + 32 + 4;
const CHUNK: [u8; 4] = *b"smKy";
/// Rows that carry a copy of the secret, starting 40 pixels in.
const ROWS: [u32; 3] = [40, 540, 1000];

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct PictureSecret([u8; 32]);

impl std::fmt::Debug for PictureSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PictureSecret(hidden)")
    }
}

impl PictureSecret {
    pub fn new() -> Result<Self> {
        let mut b = [0u8; 32];
        getrandom::getrandom(&mut b).map_err(|e| anyhow!("no randomness available: {e}"))?;
        Ok(Self(b))
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    /// For keeping it in the OS credential store on a trusted computer.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.len() != 64 {
            return None;
        }
        let mut b = [0u8; 32];
        for (i, out) in b.iter_mut().enumerate() {
            *out = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
        }
        Some(Self(b))
    }
}

fn payload(secret: &PictureSecret) -> [u8; PAYLOAD_LEN] {
    let mut p = [0u8; PAYLOAD_LEN];
    p[..4].copy_from_slice(MAGIC);
    p[4] = VERSION;
    p[5..37].copy_from_slice(&secret.0);
    let check = Sha256::digest(&p[..37]);
    p[37..].copy_from_slice(&check[..4]);
    p
}

fn parse_payload(p: &[u8]) -> Option<PictureSecret> {
    if p.len() != PAYLOAD_LEN || &p[..4] != MAGIC || p[4] != VERSION {
        return None;
    }
    if Sha256::digest(&p[..37])[..4] != p[37..] {
        return None;
    }
    let mut s = [0u8; 32];
    s.copy_from_slice(&p[5..37]);
    Some(PictureSecret(s))
}

/// Small deterministic generator for the stars, seeded with fresh randomness per picture.
struct Rng {
    seed: [u8; 32],
    counter: u64,
    buf: Vec<u8>,
}

impl Rng {
    fn next(&mut self) -> u32 {
        if self.buf.len() < 4 {
            let mut h = Sha256::new();
            h.update(self.seed);
            h.update(self.counter.to_le_bytes());
            self.counter += 1;
            self.buf.extend_from_slice(&h.finalize());
        }
        let v = u32::from_le_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]);
        self.buf.drain(..4);
        v
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Draws the picture and hides the secret in it. Returns PNG bytes.
pub fn make_picture(secret: &PictureSecret) -> Result<Vec<u8>> {
    let (w, h) = (W as usize, H as usize);
    let mut px = vec![0u8; w * h * 3];
    // Sompio's colours: the dark of the icon, a white and a green half
    let top = [14.0, 21.0, 28.0];
    let bottom = [31.0, 45.0, 58.0];
    let ice = [255.0, 255.0, 255.0];
    let amber = [76.0, 175.0, 80.0];
    // The polar sun: two half discs split at the horizon and pulled apart.
    let (r, ax, ay, bx, by) = (300.0f32, 480.0f32, 600.0f32, 600.0f32, 640.0f32);

    for y in 0..h {
        let sky = mix(top, bottom, y as f32 / h as f32);
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut c = sky;
            if fy <= ay {
                let d = ((fx - ax).powi(2) + (fy - ay).powi(2)).sqrt();
                let cover = (r - d + 0.5).clamp(0.0, 1.0) * (ay - fy + 0.5).clamp(0.0, 1.0);
                c = mix(c, ice, cover);
            }
            if fy >= by {
                let d = ((fx - bx).powi(2) + (fy - by).powi(2)).sqrt();
                let cover = (r - d + 0.5).clamp(0.0, 1.0) * (fy - by + 0.5).clamp(0.0, 1.0);
                c = mix(c, amber, cover);
            }
            let i = (y * w + x) * 3;
            px[i] = c[0].round() as u8;
            px[i + 1] = c[1].round() as u8;
            px[i + 2] = c[2].round() as u8;
        }
    }

    // Stars, different for every picture
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|e| anyhow!("no randomness available: {e}"))?;
    let mut rng = Rng { seed, counter: 0, buf: Vec::new() };
    for _ in 0..240 {
        let x = rng.below(W) as usize;
        let y = rng.below(560) as usize;
        let inside_sun = ((x as f32 - ax).powi(2) + (y as f32 - ay).powi(2)).sqrt() < r + 14.0 && (y as f32) < ay;
        if inside_sun {
            continue;
        }
        let light = 110 + rng.below(146) as u8;
        let size = if rng.below(9) == 0 { 2 } else { 1 };
        for dy in 0..size {
            for dx in 0..size {
                let (sx, sy) = (x + dx, y + dy);
                if sx < w && sy < h {
                    let i = (sy * w + sx) * 3;
                    px[i] = px[i].max(light);
                    px[i + 1] = px[i + 1].max(light);
                    px[i + 2] = px[i + 2].max(light.saturating_add(8));
                }
            }
        }
    }

    // Hide the secret: lowest bit of blue, three rows
    let data = payload(secret);
    for &row in &ROWS {
        for bit in 0..PAYLOAD_LEN * 8 {
            let byte = data[bit / 8];
            let v = (byte >> (7 - bit % 8)) & 1;
            let i = (row as usize * w + 40 + bit) * 3 + 2;
            px[i] = (px[i] & 0xFE) | v;
        }
    }

    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, W, H);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.add_text_chunk("Title".into(), "Sompio key picture".into())?;
        enc.add_text_chunk(
            "Description".into(),
            "Keep this picture private. With your password it unlocks Sompio.".into(),
        )?;
        let mut writer = enc.write_header()?;
        writer.write_chunk(png::chunk::ChunkType(CHUNK), &data)?;
        writer.write_image_data(&px)?;
        writer.finish()?;
    }
    Ok(out)
}

/// Our private chunk, read straight from the PNG structure.
fn from_chunk(bytes: &[u8]) -> Option<PictureSecret> {
    let mut i = 8; // after the signature
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[i..i + 4].try_into().ok()?) as usize;
        let kind = &bytes[i + 4..i + 8];
        let start = i + 8;
        let end = start.checked_add(len)?;
        if end + 4 > bytes.len() {
            return None;
        }
        if kind == CHUNK {
            return parse_payload(&bytes[start..end]);
        }
        if kind == b"IEND" {
            return None;
        }
        i = end + 4;
    }
    None
}

fn from_pixels(bytes: &[u8]) -> Option<PictureSecret> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        _ => return None,
    };
    if info.width != W || info.height != H {
        return None;
    }
    for &row in &ROWS {
        let mut data = [0u8; PAYLOAD_LEN];
        for bit in 0..PAYLOAD_LEN * 8 {
            let i = (row as usize * info.width as usize + 40 + bit) * channels + 2;
            let v = buf.get(i)? & 1;
            data[bit / 8] |= v << (7 - bit % 8);
        }
        if let Some(s) = parse_payload(&data) {
            return Some(s);
        }
    }
    None
}

/// Reads the secret back out of a key picture.
pub fn read_picture(bytes: &[u8]) -> Result<PictureSecret> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        bail!("That isn't the key picture: it's not a PNG file. Use the original picture Sompio saved, not a screenshot or a photo of it.");
    }
    from_chunk(bytes).or_else(|| from_pixels(bytes)).ok_or_else(|| {
        anyhow!("This picture doesn't hold a Sompio key. If it went through a chat app it was probably shrunk; use the original file.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_unique() {
        let s = PictureSecret::new().unwrap();
        let png = make_picture(&s).unwrap();
        assert_eq!(read_picture(&png).unwrap().as_bytes(), s.as_bytes());
        let png2 = make_picture(&s).unwrap();
        assert_ne!(png, png2, "stars differ every time");
        assert_eq!(PictureSecret::from_hex(&s.to_hex()).unwrap().as_bytes(), s.as_bytes());
    }

    #[test]
    fn survives_losing_the_chunk() {
        let s = PictureSecret::new().unwrap();
        let png = make_picture(&s).unwrap();
        // Re-encode the pixels as RGBA without our chunk, like an app that re-saves the file.
        let mut dec = png::Decoder::new(std::io::Cursor::new(&png));
        dec.set_transformations(png::Transformations::EXPAND);
        let mut r = dec.read_info().unwrap();
        let mut buf = vec![0u8; r.output_buffer_size().unwrap()];
        r.next_frame(&mut buf).unwrap();
        let rgba: Vec<u8> = buf.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, W, H);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&rgba).unwrap();
        }
        assert!(from_chunk(&out).is_none());
        assert_eq!(read_picture(&out).unwrap().as_bytes(), s.as_bytes());
    }

    #[test]
    fn other_files_are_refused() {
        assert!(read_picture(b"not a png").unwrap_err().to_string().contains("not a PNG"));
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 10, 10);
            enc.set_color(png::ColorType::Rgb);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[0u8; 300]).unwrap();
        }
        assert!(read_picture(&out).unwrap_err().to_string().contains("doesn't hold"));
    }
}
