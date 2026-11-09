//! New passwords and passphrases, from the operating system's secure random source, and how
//! strong a password is (zxcvbn, the estimator KeePassXC also uses).

use anyhow::{bail, Result};
use rand::rngs::OsRng;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};

const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!#$%&*+-=?@^_~.,:;/\\|()[]{}<>\"'`";
const LOOKALIKES: &str = "0O1lI|5S8B";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PasswordRules {
    pub length: usize,
    pub lower: bool,
    pub upper: bool,
    pub digits: bool,
    pub symbols: bool,
    /// Leave out characters that are easy to mix up (0 and O, 1 and l…)
    pub no_lookalikes: bool,
    /// At least one of each chosen kind
    pub every_kind: bool,
    /// Extra characters to leave out
    pub exclude: String,
}

impl Default for PasswordRules {
    fn default() -> Self {
        Self { length: 20, lower: true, upper: true, digits: true, symbols: true, no_lookalikes: true, every_kind: true, exclude: String::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PassphraseRules {
    pub words: usize,
    pub separator: String,
    pub capitalize: bool,
    /// Add one digit to a random word
    pub number: bool,
}

impl Default for PassphraseRules {
    fn default() -> Self {
        Self { words: 6, separator: "-".into(), capitalize: false, number: false }
    }
}

pub fn password(r: &PasswordRules) -> Result<String> {
    if !(4..=256).contains(&r.length) {
        bail!("Choose a length between 4 and 256.");
    }
    let skip = |c: char| r.exclude.contains(c) || (r.no_lookalikes && LOOKALIKES.contains(c));
    let kinds: Vec<Vec<char>> = [(r.lower, LOWER), (r.upper, UPPER), (r.digits, DIGITS), (r.symbols, SYMBOLS)]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, set)| set.chars().filter(|c| !skip(*c)).collect::<Vec<_>>())
        .filter(|s| !s.is_empty())
        .collect();
    if kinds.is_empty() {
        bail!("Choose at least one kind of character.");
    }
    if r.every_kind && kinds.len() > r.length {
        bail!("The password is too short to hold every kind of character.");
    }
    let all: Vec<char> = kinds.concat();
    let mut rng = OsRng;
    let mut out: Vec<char> = Vec::with_capacity(r.length);
    if r.every_kind {
        for k in &kinds {
            out.push(*k.choose(&mut rng).expect("non-empty"));
        }
    }
    while out.len() < r.length {
        out.push(*all.choose(&mut rng).expect("non-empty"));
    }
    out.shuffle(&mut rng);
    Ok(out.into_iter().collect())
}

/// Words from the EFF's long list (7,776 words, about 12.9 bits each).
pub fn passphrase(r: &PassphraseRules) -> Result<String> {
    if !(3..=20).contains(&r.words) {
        bail!("Choose between 3 and 20 words.");
    }
    let list = eff_wordlist::large::LIST;
    let mut rng = OsRng;
    let mut words: Vec<String> = (0..r.words)
        .map(|_| {
            let w = list[rng.gen_range(0..list.len())].1;
            if r.capitalize {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
            } else {
                w.to_string()
            }
        })
        .collect();
    if r.number {
        let i = rng.gen_range(0..words.len());
        words[i].push(char::from(b'0' + rng.gen_range(0..10u8)));
    }
    Ok(words.join(&r.separator))
}

#[derive(Debug, Clone, Serialize)]
pub struct Strength {
    /// 0 (very weak) to 4 (very strong)
    pub score: u8,
    pub label: &'static str,
    pub bits: f64,
    pub crack_time: String,
    pub warning: Option<String>,
    pub suggestions: Vec<String>,
}

pub fn strength(pw: &str) -> Strength {
    if pw.is_empty() {
        return Strength { score: 0, label: "Empty", bits: 0.0, crack_time: "instantly".into(), warning: None, suggestions: vec![] };
    }
    let z = zxcvbn::zxcvbn(pw, &[]);
    let score = u8::from(z.score());
    let fb = z.feedback();
    Strength {
        score,
        label: ["Very weak", "Weak", "Fair", "Strong", "Very strong"][score as usize],
        bits: z.guesses_log10() * std::f64::consts::LOG2_10,
        crack_time: z.crack_times().offline_slow_hashing_1e4_per_second().to_string(),
        warning: fb.and_then(|f| f.warning()).map(|w| w.to_string()),
        suggestions: fb.map(|f| f.suggestions().iter().map(|s| s.to_string()).collect()).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_follow_the_rules() {
        let r = PasswordRules { length: 12, symbols: false, ..Default::default() };
        for _ in 0..50 {
            let p = password(&r).unwrap();
            assert_eq!(p.chars().count(), 12);
            assert!(p.chars().any(|c| c.is_ascii_lowercase()) && p.chars().any(|c| c.is_ascii_uppercase()) && p.chars().any(|c| c.is_ascii_digit()));
            assert!(!p.chars().any(|c| LOOKALIKES.contains(c) || SYMBOLS.contains(c)));
        }
        assert!(password(&PasswordRules { lower: false, upper: false, digits: false, symbols: false, ..Default::default() }).is_err());
        let p = passphrase(&PassphraseRules { words: 5, separator: " ".into(), capitalize: true, number: true }).unwrap();
        assert_eq!(p.split(' ').count(), 5);
        assert!(p.chars().any(|c| c.is_ascii_digit()));
    }

    #[test]
    fn strength_tells_weak_from_strong() {
        assert!(strength("password1").score <= 1);
        assert!(strength(&passphrase(&PassphraseRules::default()).unwrap()).score >= 3);
    }
}
