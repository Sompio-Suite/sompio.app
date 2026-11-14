//! Sample exports for every platform Clean reads, so testers can try each import without
//! handing over their own data. Everything in them is made up: the people, numbers and
//! addresses are fictional (card 4111 1111 1111 1111 and IBAN NL91 ABNA 0417 1643 00 are the
//! standard test values).

use anyhow::Result;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SampleFile {
    pub file: String,
    pub platform: &'static str,
    pub what: &'static str,
}

fn zip(path: &Path, files: &[(&str, String)]) -> Result<()> {
    let mut z = zip::ZipWriter::new(std::fs::File::create(path)?);
    let opts = zip::write::SimpleFileOptions::default();
    for (name, body) in files {
        z.start_file(*name, opts)?;
        z.write_all(body.as_bytes())?;
    }
    z.finish()?;
    Ok(())
}

/// Writes every sample into `dir`. Returns what was written, for the "Samples" screen.
pub fn write_samples(dir: &Path) -> Result<Vec<SampleFile>> {
    std::fs::create_dir_all(dir)?;
    let mut out = Vec::new();
    let mut add = |file: &str, platform: &'static str, what: &'static str| out.push(SampleFile { file: file.into(), platform, what });

    // Discord: a data package with a server channel and two DMs
    zip(
        &dir.join("Discord package.zip"),
        &[
            ("Account/user.json", r#"{"id": "4000", "username": "alex.rivers"}"#.into()),
            ("Messages/index.json", r#"{"111": "general in Amsterdam Expats", "112": "marketplace in Amsterdam Expats", "222": "Direct Message with sam.k", "333": "Direct Message with mara"}"#.into()),
            ("Messages/c111/channel.json", r#"{"id": "111", "type": 0, "name": "general", "guild": {"id": "999", "name": "Amsterdam Expats"}}"#.into()),
            (
                "Messages/c111/messages.json",
                r#"[{"ID": "800000000000000000", "Timestamp": "2021-03-01 10:00:00", "Contents": "anyone know a good dentist near the centre?", "Attachments": ""},
                    {"ID": "800000000000000001", "Timestamp": "2021-03-01 10:02:00", "Contents": "add me, my email is alex.rivers@example.org", "Attachments": ""},
                    {"ID": "900000000000000000", "Timestamp": "2021-09-14 21:10:00", "Contents": "my new setup", "Attachments": "https://cdn.discordapp.com/attachments/1/2/desk.png"},
                    {"ID": "1000000000000000000", "Timestamp": "2022-06-02 18:00:00", "Contents": "server is at 81.204.37.12 if you want to join", "Attachments": ""}]"#.into(),
            ),
            ("Messages/c112/channel.json", r#"{"id": "112", "type": 0, "name": "marketplace", "guild": {"id": "999", "name": "Amsterdam Expats"}}"#.into()),
            (
                "Messages/c112/messages.json",
                r#"[{"ID": "1050000000000000000", "Timestamp": "2022-10-10 12:00:00", "Contents": "selling my old GPU, pay to NL91 ABNA 0417 1643 00", "Attachments": ""},
                    {"ID": "1050000000000000001", "Timestamp": "2022-10-10 12:05:00", "Contents": "or call me on 06-12345678", "Attachments": ""}]"#.into(),
            ),
            ("Messages/c222/channel.json", r#"{"id": "222", "type": 1, "recipients": ["4000", "5000"]}"#.into()),
            (
                "Messages/c222/messages.json",
                r#"[{"ID": "1100000000000000000", "Timestamp": "2023-01-01 00:00:00", "Contents": "my card is 4111 1111 1111 1111, can you book it?", "Attachments": ""},
                    {"ID": "1100000000000000001", "Timestamp": "2023-01-01 00:01:00", "Contents": "I live at Prinsengracht 263, Amsterdam", "Attachments": ""},
                    {"ID": "1100000000000000002", "Timestamp": "2023-01-01 00:02:00", "Contents": "wifi password: OwlNest2023!", "Attachments": ""}]"#.into(),
            ),
            ("Messages/c333/channel.json", r#"{"id": "333", "type": 1, "recipients": ["4000", "6000"]}"#.into()),
            (
                "Messages/c333/messages.json",
                r#"[{"ID": "1200000000000000000", "Timestamp": "2023-05-05 09:00:00", "Contents": "happy birthday! born 14/02/1994 right?", "Attachments": ""},
                    {"ID": "1200000000000000001", "Timestamp": "2023-05-05 09:01:00", "Contents": "see you saturday", "Attachments": ""},
                    {"ID": "1200000000000000002", "Timestamp": "2023-05-06 11:00:00", "Contents": "parked outside, grey car GZ-123-K", "Attachments": ""},
                    {"ID": "1200000000000000003", "Timestamp": "2023-05-07 15:30:00", "Contents": "come over https://www.google.com/maps/@52.3731234,4.8903456,17z", "Attachments": ""},
                    {"ID": "1200000000000000004", "Timestamp": "2023-06-01 22:10:00", "Contents": "saving these here, discord backup codes: 8f3k-2m9x 7h2p-4q8z", "Attachments": ""}]"#.into(),
            ),
        ],
    )?;
    add("Discord package.zip", "Discord", "A data package: a server with two channels and two DMs, with an email, phone, IBAN, card number, address, password, IP address, date of birth, number plate, map link and backup codes in it.");

    zip(
        &dir.join("Reddit export.zip"),
        &[
            ("statistics.csv", "statistic,value\naccount name,alex_rivers\nexport time,2026-11-01\n".into()),
            (
                "posts.csv",
                "id,permalink,date,ip,subreddit,gildings,title,url,body\n\
                 p1,/r/homelab/comments/p1/my_setup/,2022-05-01 10:00:00 UTC,,homelab,0,My setup,,\"Server at 81.204.37.12, ask me anything\"\n\
                 p2,/r/Netherlands/comments/p2/moving/,2023-08-01 10:00:00 UTC,,Netherlands,0,Moving tips?,,\"Moving to Keizersgracht 123 next month\"\n"
                    .into(),
            ),
            (
                "comments.csv",
                "id,permalink,date,ip,subreddit,gildings,link,parent,body,media\n\
                 c1,/r/Amsterdam/comments/x/_/c1/,2023-02-03 04:05:06 UTC,,Amsterdam,0,,,\"DM me, alex.rivers@example.org\",\n\
                 c2,/r/Amsterdam/comments/x/_/c2/,2023-02-04 04:05:06 UTC,,Amsterdam,0,,,just chatting,\n\
                 c3,/r/travel/comments/y/_/c3/,2024-04-04 04:05:06 UTC,,travel,0,,,\"is passport number: NXC4R9P12 ok for the form? new one\",\n"
                    .into(),
            ),
            ("messages.csv", "id,permalink,thread_id,date,ip,from,to,subject,body\nm1,/message/messages/m1,,2023-01-01 00:00:00 UTC,,alex_rivers,buyer1,Re: GPU,\"pay me at NL91 ABNA 0417 1643 00\"\n".into()),
        ],
    )?;
    add("Reddit export.zip", "Reddit", "Posts, comments and a private message, with an IP address, street address, email, IBAN and passport number.");

    let tg = r#"{
      "about": "Here is the data you requested.",
      "personal_information": {"user_id": 4242, "first_name": "Alex"},
      "chats": {"about": "", "list": [
        {"name": "Mara", "type": "personal_chat", "id": 777, "messages": [
          {"id": 1, "type": "message", "date": "2024-01-01T10:00:00", "date_unixtime": "1704103200", "from": "Alex", "from_id": "user4242",
           "text": ["call me on ", {"type": "phone", "text": "+31 6 12345678"}, " tonight"]},
          {"id": 2, "type": "message", "date": "2024-01-01T10:01:00", "date_unixtime": "1704103260", "from": "Mara", "from_id": "user9", "text": "only Alex's own messages are imported, not this one"},
          {"id": 3, "type": "message", "date": "2024-01-02T09:00:00", "date_unixtime": "1704186000", "from": "Alex", "from_id": "user4242", "text": "my BSN is 111222333 for the form"}
        ]},
        {"name": "Builders", "type": "private_supergroup", "id": 1500100200, "messages": [
          {"id": 55, "type": "message", "date": "2024-02-02T12:00:00", "date_unixtime": "1706875200", "from_id": "user4242", "text": "my password: Hunter2forreal", "photo": "photos/photo_1.jpg"},
          {"id": 56, "type": "message", "date": "2024-03-03T12:00:00", "date_unixtime": "1709467200", "from_id": "user4242", "text": "new wallet, seed phrase: abandon ability able about above absent absorb abstract absurd abuse access accident"}
        ]}
      ]}
    }"#;
    zip(&dir.join("Telegram export.zip"), &[("Telegram Desktop/DataExport_2026-11-01/result.json", tg.into())])?;
    add("Telegram export.zip", "Telegram", "A full Telegram Desktop export: a private chat and a group, with a phone number, a BSN, a password and a wallet's recovery phrase.");

    zip(
        &dir.join("X (Twitter) archive.zip"),
        &[
            ("data/account.js", r#"window.YTD.account.part0 = [{"account": {"accountId": "100", "username": "alexrivers"}}]"#.into()),
            (
                "data/tweets.js",
                r#"window.YTD.tweets.part0 = [
                  {"tweet": {"id_str": "1500000000000000001", "full_text": "new job! email me at alex.work@example.org", "created_at": "Thu Mar 04 05:06:07 +0000 2021"}},
                  {"tweet": {"id_str": "1500000000000000002", "full_text": "RT @someone: reposts are left out", "created_at": "Thu Mar 04 06:06:07 +0000 2021"}}
                ]"#
                .into(),
            ),
            (
                "data/direct-messages.js",
                r#"window.YTD.direct_messages.part0 = [{"dmConversation": {"conversationId": "100-200", "messages": [
                  {"messageCreate": {"id": "9001", "senderId": "100", "recipientId": "200", "text": "my address is Prinsengracht 263", "createdAt": "2022-01-01T00:00:00.000Z"}}
                ]}}]"#
                .into(),
            ),
        ],
    )?;
    add("X (Twitter) archive.zip", "X", "Posts and a sent DM, with an email and an address. Reposts are left out.");

    zip(
        &dir.join("Facebook export.zip"),
        &[
            ("profile_information/profile_information.json", r#"{"profile_v2": {"name": {"full_name": "Alex Rivers"}}}"#.into()),
            (
                "your_facebook_activity/messages/inbox/sam_123/message_1.json",
                r#"{"participants": [{"name": "Sam"}, {"name": "Alex Rivers"}], "title": "Sam",
                    "messages": [{"sender_name": "Sam", "timestamp_ms": 1700000000000, "content": "not yours, left out"},
                                 {"sender_name": "Alex Rivers", "timestamp_ms": 1700000001000, "content": "my number is 06 12345678"}]}"#
                    .into(),
            ),
            ("your_facebook_activity/posts/your_posts__check_ins__photos_and_videos_1.json", r#"[{"timestamp": 1600000000, "data": [{"post": "home sweet home at Keizersgracht 123"}]}]"#.into()),
        ],
    )?;
    add("Facebook export.zip", "Facebook", "A message and a post, with a phone number and an address.");

    zip(
        &dir.join("Instagram export.zip"),
        &[
            (
                "your_instagram_activity/messages/inbox/sam_1/message_1.json",
                r#"{"participants": [{"name": "sam.k"}, {"name": "alex.rivers"}], "title": "sam.k",
                    "messages": [{"sender_name": "alex.rivers", "timestamp_ms": 1690000000000, "content": "send it to alex.rivers@example.org"}, {"sender_name": "sam.k", "timestamp_ms": 1690000001000, "content": "ok"}]}"#
                    .into(),
            ),
            (
                "your_instagram_activity/messages/inbox/mara_2/message_1.json",
                r#"{"participants": [{"name": "mara"}, {"name": "alex.rivers"}], "title": "mara",
                    "messages": [{"sender_name": "alex.rivers", "timestamp_ms": 1690000100000, "content": "see you at the gym"}]}"#
                    .into(),
            ),
            ("your_instagram_activity/comments/post_comments_1.json", r#"[{"string_map_data": {"Comment": {"value": "congrats!!"}, "Time": {"timestamp": 1650000000}}}]"#.into()),
        ],
    )?;
    add("Instagram export.zip", "Instagram", "Two chats and a comment. Trace works out which name is yours.");

    std::fs::write(
        dir.join("user_data_tiktok.json"),
        r#"{"Profile": {"Profile Information": {"ProfileMap": {"userName": "alexr", "bioDescription": "business: alex.rivers@example.org"}}},
            "Comment": {"Comments": {"CommentsList": [{"date": "2024-01-02 10:00:00", "comment": "so true"}]}},
            "Direct Messages": {"Chat History": {"ChatHistory": {"Chat History with sam:": [
                {"Date": "2024-01-03 10:00:00", "From": "alexr", "Content": "my snap is alexr94"},
                {"Date": "2024-01-03 10:01:00", "From": "sam", "Content": "theirs"}]}}}}"#,
    )?;
    add("user_data_tiktok.json", "TikTok", "Your bio, a comment and a sent DM.");

    zip(
        &dir.join("Snapchat mydata.zip"),
        &[(
            "json/chat_history.json",
            r#"{"sam": [{"From": "me", "IsSender": true, "Content": "my new number is 06-55512345, save it", "Created": "2024-01-01 10:00:00 UTC", "Conversation Title": null},
                        {"From": "sam", "IsSender": false, "Content": "theirs", "Created": "2024-01-01 10:01:00 UTC"}]}"#
                .into(),
        )],
    )?;
    add("Snapchat mydata.zip", "Snapchat", "Chat history; only what you sent is imported.");

    zip(
        &dir.join("LinkedIn export.zip"),
        &[
            ("Profile.csv", "First Name,Last Name,Headline\nAlex,Rivers,Support Engineer\n".into()),
            ("messages.csv", "CONVERSATION ID,CONVERSATION TITLE,FROM,SENDER PROFILE URL,TO,DATE,SUBJECT,CONTENT\nc1,,Alex Rivers,x,Sam,2024-01-01 10:00:00 UTC,,my number +31 6 12345678\n".into()),
            ("Shares.csv", "Date,ShareLink,ShareCommentary\n2023-05-01 09:00:00,https://www.linkedin.com/feed/update/1,\"Excited to share I'm starting at Example BV\"\n".into()),
            ("Comments.csv", "Date,Link,Message\n2023-06-01 09:00:00,https://www.linkedin.com/feed/update/2,Congrats!\n".into()),
        ],
    )?;
    add("LinkedIn export.zip", "LinkedIn", "A message, a post and a comment.");

    std::fs::write(
        dir.join("WhatsApp Chat with Sam.txt"),
        "12/03/2024, 09:15 - Messages and calls are end-to-end encrypted.\n12/03/2024, 09:16 - Sam: hi, what's your address?\n13/03/2024, 09:17 - Alex: Prinsengracht 263, 1016 GV Amsterdam\n13/03/2024, 09:18 - Sam: thanks\n13/03/2024, 09:20 - Alex: and my IBAN for the tickets: NL91 ABNA 0417 1643 00\n",
    )?;
    add("WhatsApp Chat with Sam.txt", "WhatsApp", "An exported chat. The file name says Sam is the other person, so Trace keeps only your lines.");

    zip(
        &dir.join("Gmail takeout.zip"),
        &[(
            "Takeout/Mail/All mail Including Spam and Trash.mbox",
            "From 1@xxx Mon Jan 1 00:00:00 2024\nX-Gmail-Labels: Sent\nMessage-ID: <m1@mail.example>\nTo: hr@example.org\nSubject: Documents\nDate: Mon, 1 Jan 2024 10:00:00 +0000\n\nHi, attached is my passport. My BSN is 111222333.\n\n\
             From 2@xxx Tue Jan 2 00:00:00 2024\nX-Gmail-Labels: Sent\nMessage-ID: <m2@mail.example>\nTo: landlord@example.org\nSubject: Rent\nDate: Tue, 2 Jan 2024 10:00:00 +0000\n\nPaid from NL91 ABNA 0417 1643 00.\n\n\
             From 3@xxx Mon Jan 1 00:00:00 2024\nX-Gmail-Labels: Inbox,Unread\nMessage-ID: <m3@mail.example>\nSubject: Sale\n\nmail you received is left out\n"
                .into(),
        )],
    )?;
    add("Gmail takeout.zip", "Email", "Google Takeout mail; only mail you sent is imported.");

    // A folder to try "Scan a folder on this computer"
    let pc = dir.join("Sample computer folder");
    std::fs::create_dir_all(pc.join("taxes/2023"))?;
    std::fs::write(pc.join("notes.txt"), "wifi password: SuperSecret99\nbank NL91 ABNA 0417 1643 00\n")?;
    std::fs::write(pc.join("taxes/2023/Passport_scan.jpg"), [0xFFu8, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0, 0, 0, 0, 0])?;
    std::fs::write(pc.join("contacts.csv"), "name,phone,email\nSam K,06-98765432,sam.k@example.org\n")?;
    std::fs::write(pc.join("holiday plans.txt"), "Nothing private in here.\n")?;
    zip(
        &pc.join("CV Alex Rivers.docx"),
        &[("word/document.xml", "<w:document><w:body><w:p><w:r><w:t>Alex Rivers</w:t></w:r></w:p><w:p><w:r><w:t>alex.rivers@example.org &amp; 06-12345678</w:t></w:r></w:p></w:body></w:document>".into())],
    )?;
    std::fs::create_dir_all(pc.join("Pictures"))?;
    std::fs::write(pc.join("Pictures/Screenshot 2024-06-01.png"), include_bytes!("../assets/screenshot-notes.png"))?;
    std::fs::write(pc.join("Pictures/IMG_2041.jpg"), include_bytes!("../assets/photo-with-location.jpg"))?;
    add(
        "Sample computer folder",
        "This computer",
        "A folder for \"Scan folders\": notes with a password and IBAN, a passport scan's file name, contacts, a CV, a screenshot with backup codes and ID numbers (tick \"Read text in pictures\") and a photo that shows where it was taken (tick \"Check where photos were taken\").",
    );

    let mut readme = String::from(
        "Sompio Trace samples\r\n====================\r\n\r\nMade-up exports to try every import in Sompio Trace. Nothing in them is real.\r\nIn Trace, open Sources, choose \"Add an export\" and pick one of these.\r\n\r\n",
    );
    for f in &out {
        readme.push_str(&format!("{}  ({})\r\n  {}\r\n\r\n", f.file, f.platform, f.what));
    }
    std::fs::write(dir.join("READ ME.txt"), readme)?;
    Ok(out)
}
