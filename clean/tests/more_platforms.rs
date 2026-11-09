//! WhatsApp, Facebook, Instagram, TikTok, Snapchat, LinkedIn and Gmail exports.

mod common;
use common::*;
use sompio_clean::{import_package, Store};

fn texts(store: &Store, platform: &str) -> Vec<String> {
    let mut out = vec![];
    for c in store.channels(None).unwrap().into_iter().filter(|c| c.platform == platform) {
        out.extend(store.messages(&c.id, 0, 1000).unwrap().items.into_iter().map(|m| store.reveal_message(&m.id).unwrap().unwrap().content));
    }
    out.sort();
    out
}

fn import(store: &mut Store, path: &std::path::Path) -> sompio_clean::ImportSummary {
    import_package(store, path, &mut |_| {}).unwrap()
}

#[test]
fn whatsapp_chat_file_keeps_only_your_lines() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("WhatsApp Chat with Sam.txt");
    std::fs::write(
        &p,
        "12/03/2024, 09:15 - Messages and calls are end-to-end encrypted.\n12/03/2024, 09:16 - Sam: hi, what's your address?\n13/03/2024, 09:17 - Alex: Prinsengracht 263\nsecond line of mine\n13/03/2024, 09:18 - Sam: thanks\n",
    )
    .unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let s = import(&mut store, &p);
    assert_eq!(s.platform, "whatsapp");
    assert_eq!(texts(&store, "whatsapp"), vec!["Prinsengracht 263\nsecond line of mine"]);
}

#[test]
fn facebook_messages_posts_and_encoding() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("facebook.zip");
    write_zip(
        &p,
        &[
            ("profile_information/profile_information.json", r#"{"profile_v2": {"name": {"full_name": "Alex Doe"}}}"#.into()),
            (
                "your_facebook_activity/messages/inbox/sam_123/message_1.json",
                r#"{"participants": [{"name": "Sam"}, {"name": "Alex Doe"}], "title": "Sam",
                    "messages": [{"sender_name": "Sam", "timestamp_ms": 1700000000000, "content": "their text"},
                                 {"sender_name": "Alex Doe", "timestamp_ms": 1700000001000, "content": "cafÃ© at 6"}]}"#
                    .into(),
            ),
            (
                "your_facebook_activity/posts/your_posts__check_ins__photos_and_videos_1.json",
                r#"[{"timestamp": 1600000000, "data": [{"post": "my phone is 06 12345678"}]}]"#.into(),
            ),
        ],
    );
    let mut store = Store::open_in_memory().unwrap();
    let s = import(&mut store, &p);
    assert_eq!(s.platform, "facebook");
    assert_eq!(texts(&store, "facebook"), vec!["café at 6", "my phone is 06 12345678"]);
}

#[test]
fn instagram_guesses_you_from_participants_and_reads_comments() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("instagram.zip");
    let chat = |who: &str| {
        format!(
            r#"{{"participants": [{{"name": "{who}"}}, {{"name": "me_gram"}}], "title": "{who}",
                "messages": [{{"sender_name": "me_gram", "timestamp_ms": 1, "content": "hi {who}"}}, {{"sender_name": "{who}", "timestamp_ms": 2, "content": "not mine"}}]}}"#
        )
    };
    write_zip(
        &p,
        &[
            ("your_instagram_activity/messages/inbox/a_1/message_1.json", chat("ann")),
            ("your_instagram_activity/messages/inbox/b_2/message_1.json", chat("bob")),
            (
                "your_instagram_activity/comments/post_comments_1.json",
                r#"[{"string_map_data": {"Comment": {"value": "nice!"}, "Time": {"timestamp": 1650000000}}}]"#.into(),
            ),
        ],
    );
    let mut store = Store::open_in_memory().unwrap();
    let s = import(&mut store, &p);
    assert_eq!(s.platform, "instagram");
    assert_eq!(texts(&store, "instagram"), vec!["hi ann", "hi bob", "nice!"]);
}

#[test]
fn tiktok_comments_bio_and_sent_dms() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("user_data_tiktok.json");
    std::fs::write(
        &p,
        r#"{"Profile": {"Profile Information": {"ProfileMap": {"userName": "alexd", "bioDescription": "mail me: a@example.org"}}},
            "Comment": {"Comments": {"CommentsList": [{"date": "2024-01-02 10:00:00", "comment": "so true"}]}},
            "Direct Messages": {"Chat History": {"ChatHistory": {"Chat History with sam:": [
                {"Date": "2024-01-03 10:00:00", "From": "alexd", "Content": "see you"},
                {"Date": "2024-01-03 10:01:00", "From": "sam", "Content": "theirs"}]}}}}"#,
    )
    .unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let s = import(&mut store, &p);
    assert_eq!(s.platform, "tiktok");
    assert_eq!(texts(&store, "tiktok"), vec!["mail me: a@example.org", "see you", "so true"]);
}

#[test]
fn snapchat_only_sent_messages() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("mydata.zip");
    write_zip(
        &p,
        &[(
            "json/chat_history.json",
            r#"{"sam": [{"From": "me", "IsSender": true, "Content": "mine", "Created": "2024-01-01 10:00:00 UTC", "Conversation Title": null},
                        {"From": "sam", "IsSender": false, "Content": "theirs", "Created": "2024-01-01 10:01:00 UTC"}]}"#
                .into(),
        )],
    );
    let mut store = Store::open_in_memory().unwrap();
    let s = import(&mut store, &p);
    assert_eq!(s.platform, "snapchat");
    assert_eq!(texts(&store, "snapchat"), vec!["mine"]);
}

#[test]
fn linkedin_messages_shares_comments() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("Basic_LinkedInDataExport.zip");
    write_zip(
        &p,
        &[
            ("Profile.csv", "First Name,Last Name,Headline\nAlex,Doe,Engineer\n".into()),
            (
                "messages.csv",
                "CONVERSATION ID,CONVERSATION TITLE,FROM,SENDER PROFILE URL,TO,DATE,SUBJECT,CONTENT\nc1,,Alex Doe,x,Sam,2024-01-01 10:00:00 UTC,,my number +31 6 12345678\nc1,,Sam,x,Alex Doe,2024-01-01 10:01:00 UTC,,theirs\n".into(),
            ),
            ("Shares.csv", "Date,ShareLink,ShareCommentary\n2023-05-01 09:00:00,https://www.linkedin.com/feed/update/1,\"Excited to share\"\n".into()),
            ("Comments.csv", "Date,Link,Message\n2023-06-01 09:00:00,https://www.linkedin.com/feed/update/2,Congrats!\n".into()),
        ],
    );
    let mut store = Store::open_in_memory().unwrap();
    let s = import(&mut store, &p);
    assert_eq!(s.platform, "linkedin");
    assert_eq!(texts(&store, "linkedin"), vec!["Congrats!", "Excited to share", "my number +31 6 12345678"]);
}

#[test]
fn gmail_takeout_imports_sent_mail_only() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("takeout.zip");
    write_zip(
        &p,
        &[(
            "Takeout/Mail/All mail Including Spam and Trash.mbox",
            "From 1@xxx Mon Jan 1 00:00:00 2024\nX-Gmail-Labels: Sent\nMessage-ID: <m1@mail.example>\nTo: hr@example.org\nSubject: Documents\nDate: Mon, 1 Jan 2024 10:00:00 +0000\n\nAttached is my passport.\n\nFrom 2@xxx Mon Jan 1 00:00:00 2024\nX-Gmail-Labels: Inbox,Unread\nMessage-ID: <m2@mail.example>\nSubject: Sale\n\nbuy now\n".into(),
        )],
    );
    let mut store = Store::open_in_memory().unwrap();
    let s = import(&mut store, &p);
    assert_eq!(s.platform, "email");
    let t = texts(&store, "email");
    assert_eq!(t.len(), 1);
    assert!(t[0].starts_with("To: hr@example.org\nSubject: Documents") && t[0].contains("passport"));
}
