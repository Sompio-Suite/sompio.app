use sompio_clean::{Kind, ScanOptions, Scanner};

fn kinds(text: &str) -> Vec<Kind> {
    let s = Scanner::new(&ScanOptions { watch_terms: vec!["Jan de Vries".into(), "Acme BV".into()] }).unwrap();
    s.detect(text).into_iter().map(|d| d.kind).collect()
}

fn has(text: &str, k: Kind) -> bool {
    kinds(text).contains(&k)
}

#[test]
fn finds_real_personal_info() {
    assert!(has("mail me at jan.devries@gmail.com", Kind::Email));
    assert!(has("bel me: +31 6 12345678", Kind::Phone));
    assert!(has("mijn nummer is 06-12345678", Kind::Phone));
    assert!(has("call (415) 555-0132 tomorrow", Kind::Phone));
    assert!(has("IBAN NL91 ABNA 0417 1643 00 thanks", Kind::Iban));
    assert!(has("NL91ABNA0417164300", Kind::Iban));
    assert!(has("card 4111 1111 1111 1111 exp 12/29", Kind::Card));
    assert!(has("I live at Prinsengracht 263, 1016 GV Amsterdam", Kind::Address));
    assert!(has("I live at Prinsengracht 263, 1016 GV Amsterdam", Kind::Postcode));
    assert!(has("ship it to 221B Baker Street please", Kind::Address));
    assert!(has("stau pe Strada Lipscani nr. 12", Kind::Address));
    assert!(has("my server is 81.204.12.7", Kind::IpAddress));
    assert!(has("password: hunter2abc", Kind::Credential));
    assert!(has("token ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789", Kind::Credential));
    assert!(has("I'm Jan de Vries btw", Kind::WatchTerm));
    assert!(has("working at ACME BV now", Kind::WatchTerm));
}

#[test]
fn ignores_common_false_alarms() {
    // Discord mentions, channel links and IDs are long numbers, not cards or phones
    assert!(kinds("hey <@123456789012345678> look").is_empty());
    assert!(kinds("https://discord.com/channels/111111111111111111/222222222222222222").is_empty());
    assert!(kinds("4111111111111112").is_empty(), "fails Luhn");
    assert!(!has("IBAN NL91 ABNA 0417 1643 01", Kind::Iban), "bad checksum");
    // private / local IPs and version numbers
    assert!(!has("router at 192.168.1.1", Kind::IpAddress));
    assert!(!has("updated to v1.2.3.4", Kind::IpAddress));
    // casual sentences
    assert!(kinds("it's 5 minutes down the road").is_empty());
    assert!(kinds("meet at 2024 AM? lol, 1080 HZ monitor").is_empty());
    assert!(kinds("gg wp, 1v1 me at 10pm").is_empty());
    // watch terms match whole words only
    assert!(!has("janitor duty", Kind::WatchTerm));
}

#[test]
fn iban_digits_are_not_also_a_phone_number() {
    let k = kinds("NL91 ABNA 0417 1643 00");
    assert_eq!(k, vec![Kind::Iban]);
}

#[test]
fn finds_identity_numbers_codes_dates_and_plates() {
    // BSN 111222333 passes the eleven test; only flagged next to its name
    assert!(has("mijn bsn is 111222333", Kind::IdNumber));
    assert!(has("BSN: 1112.22.333", Kind::IdNumber));
    assert!(!has("order 111222333 shipped", Kind::IdNumber));
    assert!(!has("bsn 111222334", Kind::IdNumber), "fails the eleven test");
    assert!(has("my ssn is 123-45-6789", Kind::IdNumber));
    assert!(!has("ssn 000-12-3456", Kind::IdNumber));
    // A valid Romanian CNP (check digit 6)
    assert!(has("CNP 1800101221144 pentru formular", Kind::IdNumber));
    assert!(!has("CNP 1800101221145", Kind::IdNumber), "bad check digit");
    assert!(has("passport number: NXC4R9P12", Kind::Passport));
    assert!(has("paspoort nr. BN12FG345", Kind::Passport));
    assert!(!has("my passport is EXPIRED", Kind::Passport));
    assert!(has("discord backup codes: 8f3k-2m9x", Kind::BackupCode));
    assert!(!has("my recovery codes are stored safely", Kind::BackupCode));
    assert!(has(
        "seed phrase: abandon ability able about above absent absorb abstract absurd abuse access accident",
        Kind::BackupCode
    ));
    assert!(!has(
        "recovery phrase is something you should never share with the people you meet online okay",
        Kind::BackupCode
    ));
    assert!(has("born on 14-03-1994 in Utrecht", Kind::BirthDate));
    assert!(has("date of birth: March 14, 1994", Kind::BirthDate));
    assert!(has("geboren op 14 maart 1994", Kind::BirthDate));
    assert!(!has("born to raid, 1994 was a good year", Kind::BirthDate));
    assert!(has("my car is GZ-123-K, parked outside", Kind::NumberPlate));
    assert!(has("kenteken: 12-ABC-3", Kind::NumberPlate));
    assert!(has("license plate is AB 123 CD", Kind::NumberPlate));
    assert!(!has("GG-WP-ME lol", Kind::NumberPlate));
}
