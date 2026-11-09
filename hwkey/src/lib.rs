//! YubiKey challenge-response (HMAC-SHA1, slot 1 or 2), the same mechanism KeePassXC uses.
//! Talks to the key's OTP interface with HID feature reports, which Windows allows without
//! admin rights or extra drivers. The protocol: a 70-byte frame (64-byte challenge, command,
//! CRC-16, filler) written as 8-byte reports, then the 20-byte HMAC read back the same way.

use anyhow::{anyhow, bail, Result};
use std::time::{Duration, Instant};

pub const YUBICO: u16 = 0x1050;
const SLOT_WRITE: u8 = 0x80;
const RESP_PENDING: u8 = 0x40;
/// Set while the key is waiting for a touch
const RESP_TIMEOUT_WAIT: u8 = 0x20;
const CRC_OK: u16 = 0xF0B8;
const CMD_SERIAL: u8 = 0x10;
const CMD_HMAC1: u8 = 0x30;
const CMD_HMAC2: u8 = 0x38;

pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= b as u16;
        for _ in 0..8 {
            let j = crc & 1;
            crc >>= 1;
            if j != 0 {
                crc ^= 0x8408;
            }
        }
    }
    crc
}

/// Reading and writing 8-byte feature reports. A real key, or a simulated one in tests.
pub trait Reports {
    fn get(&mut self) -> Result<[u8; 8]>;
    fn set(&mut self, report: [u8; 8]) -> Result<()>;
}

fn frame(payload: [u8; 64], cmd: u8) -> [u8; 70] {
    let mut f = [0u8; 70];
    f[..64].copy_from_slice(&payload);
    f[64] = cmd;
    f[65..67].copy_from_slice(&crc16(&payload).to_le_bytes());
    f
}

fn wait<R: Reports>(dev: &mut R, deadline: Instant, done: impl Fn(u8) -> bool, on_touch: &mut dyn FnMut()) -> Result<[u8; 8]> {
    let mut told = false;
    loop {
        let r = dev.get()?;
        if done(r[7]) {
            return Ok(r);
        }
        if r[7] & RESP_TIMEOUT_WAIT != 0 && !told {
            told = true;
            on_touch();
        }
        if Instant::now() > deadline {
            bail!("The YubiKey didn't answer. If it's blinking, touch it; otherwise unplug it, plug it back in and try again.");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Sends a command and returns the response bytes (up to 36).
fn exchange<R: Reports>(dev: &mut R, payload: [u8; 64], cmd: u8, on_touch: &mut dyn FnMut()) -> Result<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let f = frame(payload, cmd);
    wait(dev, deadline, |s| s & SLOT_WRITE == 0, on_touch)?;
    for (seq, chunk) in f.chunks(7).enumerate() {
        // All-zero chunks in the middle can be skipped, as the key fills them in
        if seq != 0 && seq != 9 && chunk.iter().all(|&b| b == 0) {
            continue;
        }
        let mut r = [0u8; 8];
        r[..7].copy_from_slice(chunk);
        r[7] = SLOT_WRITE | seq as u8;
        wait(dev, deadline, |s| s & SLOT_WRITE == 0, on_touch)?;
        dev.set(r)?;
    }
    // Wait for the answer (after a touch, if the slot asks for one)
    let first = wait(dev, deadline, |s| s & RESP_PENDING != 0, on_touch)?;
    let mut out = first[..7].to_vec();
    loop {
        let r = dev.get()?;
        if r[7] & RESP_PENDING == 0 || (r[7] & 0x1F) == 0 {
            break;
        }
        out.extend_from_slice(&r[..7]);
        if out.len() >= 42 {
            break;
        }
    }
    // Reset the read state
    dev.set([0, 0, 0, 0, 0, 0, 0, 0x8F])?;
    Ok(out)
}

/// HMAC-SHA1 of the challenge with the secret in the given slot (1 or 2).
pub fn hmac<R: Reports>(dev: &mut R, slot: u8, challenge: &[u8; 32], on_touch: &mut dyn FnMut()) -> Result<[u8; 20]> {
    // Pad like KeePassXC: the rest of the 64 bytes hold the padding length, which works with
    // both fixed 64-byte and variable-length slots.
    let mut payload = [32u8; 64];
    payload[..32].copy_from_slice(challenge);
    let cmd = if slot == 1 { CMD_HMAC1 } else { CMD_HMAC2 };
    let r = exchange(dev, payload, cmd, on_touch)?;
    if r.len() < 22 || crc16(&r[..22]) != CRC_OK {
        bail!("The YubiKey's answer didn't check out. Is slot {slot} set up for challenge-response (HMAC-SHA1)?");
    }
    let mut out = [0u8; 20];
    out.copy_from_slice(&r[..20]);
    Ok(out)
}

pub fn serial<R: Reports>(dev: &mut R) -> Result<u32> {
    let r = exchange(dev, [0u8; 64], CMD_SERIAL, &mut || {})?;
    if r.len() < 6 || crc16(&r[..6]) != CRC_OK {
        bail!("Couldn't read the YubiKey's serial number");
    }
    Ok(u32::from_be_bytes([r[0], r[1], r[2], r[3]]))
}

/// A YubiKey plugged into this PC.
pub struct Device {
    dev: hidapi::HidDevice,
}

impl Reports for Device {
    fn get(&mut self) -> Result<[u8; 8]> {
        let mut buf = [0u8; 9];
        let n = self.dev.get_feature_report(&mut buf).map_err(|e| anyhow!("Couldn't read from the YubiKey: {e}"))?;
        if n < 8 {
            bail!("The YubiKey sent a short answer");
        }
        // Some systems include the report id, some don't
        let start = if n >= 9 { 1 } else { 0 };
        let mut out = [0u8; 8];
        out.copy_from_slice(&buf[start..start + 8]);
        Ok(out)
    }
    fn set(&mut self, report: [u8; 8]) -> Result<()> {
        let mut buf = [0u8; 9];
        buf[1..].copy_from_slice(&report);
        self.dev.send_feature_report(&buf).map_err(|e| anyhow!("Couldn't write to the YubiKey: {e}"))
    }
}

#[derive(Debug, Clone)]
pub struct Found {
    pub name: String,
    pub serial: Option<u32>,
}

fn otp_interfaces(api: &hidapi::HidApi) -> Vec<&hidapi::DeviceInfo> {
    let mut seen = std::collections::HashSet::new();
    api.device_list()
        .filter(|d| d.vendor_id() == YUBICO)
        // The OTP interface shows up as a keyboard (usage page 1, usage 6)
        .filter(|d| (d.usage_page() == 1 && d.usage() == 6) || d.interface_number() == 0)
        .filter(|d| seen.insert(d.path().to_owned()))
        .collect()
}

/// The YubiKeys plugged in, with their serial numbers where they share them.
pub fn list() -> Result<Vec<Found>> {
    let api = hidapi::HidApi::new().map_err(|e| anyhow!("USB devices can't be read here: {e}"))?;
    let mut out = Vec::new();
    for info in otp_interfaces(&api) {
        let name = info.product_string().unwrap_or("YubiKey").to_string();
        let serial = info.open_device(&api).ok().and_then(|dev| serial(&mut Device { dev }).ok());
        if !out.iter().any(|f: &Found| f.serial.is_some() && f.serial == serial) {
            out.push(Found { name, serial });
        }
    }
    Ok(out)
}

/// Challenge-response with the first YubiKey that answers (or the one with this serial).
pub fn challenge(slot: u8, challenge_bytes: &[u8; 32], want_serial: Option<u32>, on_touch: &mut dyn FnMut()) -> Result<([u8; 20], Option<u32>)> {
    let api = hidapi::HidApi::new().map_err(|e| anyhow!("USB devices can't be read here: {e}"))?;
    let infos = otp_interfaces(&api);
    if infos.is_empty() {
        bail!("No YubiKey found. Plug it in and try again.");
    }
    let mut last = None;
    for info in infos {
        let Ok(dev) = info.open_device(&api) else { continue };
        let mut d = Device { dev };
        let s = serial(&mut d).ok();
        if want_serial.is_some() && s.is_some() && s != want_serial {
            continue;
        }
        match hmac(&mut d, slot, challenge_bytes, on_touch) {
            Ok(r) => return Ok((r, s)),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("The YubiKey plugged in isn't the one added to this Sompio.")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmac::{Hmac, Mac};

    /// A simulated YubiKey following the documented protocol, with an HMAC secret in slot 2.
    struct Sim {
        secret: [u8; 20],
        buf: [u8; 70],
        status: u8,
        out: Vec<[u8; 8]>,
        pending: usize,
        touch_polls: u32,
    }

    impl Sim {
        fn new() -> Self {
            Sim { secret: *b"0123456789abcdefghij", buf: [0; 70], status: 0, out: vec![], pending: 0, touch_polls: 3 }
        }
        fn respond(&mut self, data: &[u8]) {
            let mut resp = data.to_vec();
            let crc = !crc16(&resp);
            resp.extend_from_slice(&crc.to_le_bytes());
            self.out = resp.chunks(7).enumerate().map(|(i, c)| {
                let mut r = [0u8; 8];
                r[..c.len()].copy_from_slice(c);
                r[7] = RESP_PENDING | i as u8;
                r
            }).collect();
            self.out.push([0, 0, 0, 0, 0, 0, 0, RESP_PENDING]);
            self.pending = 0;
        }
    }

    impl Reports for Sim {
        fn get(&mut self) -> Result<[u8; 8]> {
            if self.touch_polls > 0 && !self.out.is_empty() && self.pending == 0 {
                self.touch_polls -= 1;
                return Ok([0, 0, 0, 0, 0, 0, 0, RESP_TIMEOUT_WAIT]);
            }
            if self.pending < self.out.len() {
                self.pending += 1;
                return Ok(self.out[self.pending - 1]);
            }
            Ok([0, 0, 0, 0, 0, 0, 0, self.status])
        }
        fn set(&mut self, r: [u8; 8]) -> Result<()> {
            if r[7] == 0x8F {
                self.out.clear();
                return Ok(());
            }
            let seq = (r[7] & 0x1F) as usize;
            if seq == 0 {
                self.buf = [0; 70];
            }
            self.buf[seq * 7..seq * 7 + 7].copy_from_slice(&r[..7]);
            if seq == 9 {
                let payload = &self.buf[..64];
                assert_eq!(u16::from_le_bytes([self.buf[65], self.buf[66]]), crc16(payload), "frame CRC");
                match self.buf[64] {
                    CMD_HMAC2 => {
                        // Variable-length mode: strip the padding (trailing bytes equal to the last)
                        let last = payload[63];
                        let len = payload.iter().rposition(|&b| b != last).map(|p| p + 1).unwrap_or(0);
                        let mut m = Hmac::<sha1::Sha1>::new_from_slice(&self.secret).unwrap();
                        m.update(&payload[..len]);
                        let h: [u8; 20] = m.finalize().into_bytes().into();
                        self.respond(&h);
                    }
                    CMD_SERIAL => self.respond(&12345678u32.to_be_bytes()),
                    other => panic!("unexpected command {other:#x}"),
                }
            }
            Ok(())
        }
    }

    #[test]
    fn crc_residual() {
        let data = b"hello";
        let mut v = data.to_vec();
        v.extend_from_slice(&(!crc16(data)).to_le_bytes());
        assert_eq!(crc16(&v), CRC_OK);
    }

    #[test]
    fn challenge_response_against_a_simulated_key() {
        let mut sim = Sim::new();
        let ch = [7u8; 32];
        let mut touched = 0;
        let r = hmac(&mut sim, 2, &ch, &mut || touched += 1).unwrap();
        let mut m = Hmac::<sha1::Sha1>::new_from_slice(b"0123456789abcdefghij").unwrap();
        m.update(&ch);
        let want: [u8; 20] = m.finalize().into_bytes().into();
        assert_eq!(r, want, "same as HMAC-SHA1 of the 32-byte challenge");
        assert_eq!(touched, 1, "asked for a touch once");
        sim.touch_polls = 0;
        assert_eq!(hmac(&mut sim, 2, &ch, &mut || {}).unwrap(), want, "deterministic");
        assert_eq!(serial(&mut sim).unwrap(), 12345678);
    }
}
