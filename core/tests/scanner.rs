use veil_core::{Kind, ScanOptions, Scanner};

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
