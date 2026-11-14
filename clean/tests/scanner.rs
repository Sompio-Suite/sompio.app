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

#[test]
fn finds_exact_locations() {
    assert!(has("[Photo location: 52.37312, 4.89034]", Kind::Location));
    assert!(has("come over https://www.google.com/maps/@52.3731234,4.8903456,17z", Kind::Location));
    assert!(has("https://www.openstreetmap.org/#map=18/52.37312/4.89034", Kind::Location));
    assert!(!has("pi is 3.14159, e is 2.71828", Kind::Location));
}

#[test]
fn finds_numbers_from_other_countries() {
    // Bank accounts
    for iban in ["DE89 3704 0044 0532 0130 00", "FR14 2004 1010 0505 0001 3M02 606", "ES91 2100 0418 4502 0005 1332", "IT60 X054 2811 1010 0000 0123 456", "GB29 NWBK 6016 1331 9268 19", "BE68 5390 0754 7034", "PL61 1090 1014 0000 0712 1981 2874", "RO49 AAAA 1B31 0075 9384 0000"] {
        assert!(has(&format!("send it to {iban} please"), Kind::Iban), "{iban}");
    }
    assert!(has("sort code 20-00-00, account number 12345678", Kind::Iban));
    assert!(has("routing number 021000021 account 1234567890", Kind::Iban));
    assert!(!has("routing number 021000022 account 1234567890", Kind::Iban), "bad check digit");
    // ID numbers
    assert!(has("my NI number is AB 12 34 56 C", Kind::IdNumber));
    assert!(!has("GB 12 34 56 A", Kind::IdNumber));
    assert!(has("DNI 12345678Z para la reserva", Kind::IdNumber));
    assert!(!has("DNI 12345678A", Kind::IdNumber), "wrong check letter");
    assert!(has("NIE X1234567L", Kind::IdNumber));
    assert!(has("codice fiscale RSSMRA85T10A562S", Kind::IdNumber));
    assert!(has("PESEL 44051401359", Kind::IdNumber));
    assert!(!has("PESEL 44051401358", Kind::IdNumber));
    assert!(has("Steuer-ID: 12 345 678 901", Kind::IdNumber));
    assert!(has("numéro de sécurité sociale 1 85 05 78 006 084 91", Kind::IdNumber));
    assert!(!has("numéro de sécurité sociale 1 85 05 78 006 084 36", Kind::IdNumber));
    // Addresses
    assert!(has("j'habite au 12 rue de Rivoli, Paris", Kind::Address));
    assert!(has("vivo en Calle Mayor 5, Madrid", Kind::Address));
    assert!(has("abito in Via Roma, 10 a Milano", Kind::Address));
    assert!(has("mieszkam na ul. Długa 7", Kind::Address));
    assert!(!has("send it via email 3 times", Kind::Address));
}
