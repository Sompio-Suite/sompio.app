//! Fake exports for each platform, shaped like the real ones.
#![allow(dead_code)]

use std::io::Write;
use std::path::Path;
use veil_core::ScanSummary;

// Discord snowflakes from 2021 and 2023.
pub const M1: &str = "800000000000000000";
pub const M2: &str = "800000000000000001";
pub const M3: &str = "1100000000000000000";

pub fn write_zip(path: &Path, files: &[(&str, String)]) {
    let f = std::fs::File::create(path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    let opts = zip::write::SimpleFileOptions::default();
    for (name, body) in files {
        z.start_file(*name, opts).unwrap();
        z.write_all(body.as_bytes()).unwrap();
    }
    z.finish().unwrap();
}

pub fn count(s: &ScanSummary, kind: &str) -> i64 {
    s.by_kind.iter().find(|c| c.kind.key() == kind).unwrap().count
}

pub fn discord_package(path: &Path) {
    write_zip(
        path,
        &[
            ("Account/user.json", "{}".into()),
            ("Activity/analytics/events-2023.json", "huge file we skip".into()),
            ("Messages/index.json", r#"{"111": "general in Test Server", "222": "Direct Message with friend"}"#.into()),
            (
                "Messages/c111/channel.json",
                r#"{"id": "111", "type": "GUILD_TEXT", "name": "general", "guild": {"id": "999", "name": "Test Server"}}"#.into(),
            ),
            (
                "Messages/c111/messages.json",
                format!(
                    r#"[{{"ID": {M1}, "Timestamp": "2021-03-01 10:00:00", "Contents": "hello world, my email is someone@example.org", "Attachments": ""}},
                        {{"ID": "{M2}", "Timestamp": "2021-03-01 10:00:01", "Contents": "cat pic", "Attachments": "https://cdn.discordapp.com/a.png"}}]"#
                ),
            ),
            ("Messages/c222/channel.json", r#"{"id": "222", "type": "DM", "recipients": ["1", "2"]}"#.into()),
            (
                "Messages/c222/messages.json",
                format!(
                    r#"[{{"ID": "{M3}", "Timestamp": "2023-01-01 00:00:00", "Contents": "my card is 4111 1111 1111 1111 and I live at Prinsengracht 263", "Attachments": ""}}]"#
                ),
            ),
        ],
    );
}

pub fn reddit_export(path: &Path) {
    write_zip(
        path,
        &[
            ("statistics.csv", "statistic,value\naccount name,sam_test\nexport time,2026-11-01\n".into()),
            (
                "posts.csv",
                "id,permalink,date,ip,subreddit,gildings,title,url,body\n\
                 p1,/r/homelab/comments/p1/my_setup/,2022-05-01 10:00:00 UTC,1.2.3.4,homelab,0,My setup,,\"Server at 81.204.37.12, ask me anything\"\n"
                    .into(),
            ),
            (
                "comments.csv",
                "id,permalink,date,ip,subreddit,gildings,link,parent,body,media\n\
                 c1,https://www.reddit.com/r/Amsterdam/comments/x/_/c1/,2023-02-03 04:05:06 UTC,,Amsterdam,0,,,\"Moving to Keizersgracht 123 next month\",\n\
                 c2,/r/Amsterdam/comments/x/_/c2/,2023-02-04 04:05:06 UTC,,Amsterdam,0,,,just chatting,\n"
                    .into(),
            ),
            (
                "messages.csv",
                "id,permalink,thread_id,date,ip,from,to,subject,body\n\
                 m1,/message/messages/m1,,2023-01-01 00:00:00 UTC,,sam_test,buyer1,Re: GPU,\"pay me at NL91 ABNA 0417 1643 00\"\n\
                 m2,/message/messages/m2,,2023-01-01 00:01:00 UTC,,buyer1,sam_test,GPU,\"my number is 06-98765432\"\n"
                    .into(),
            ),
        ],
    );
}

pub fn telegram_export(path: &Path) {
    let json = r#"{
      "about": "Here is the data you requested.",
      "personal_information": {"user_id": 4242, "first_name": "Sam"},
      "chats": {"about": "", "list": [
        {"name": "Mara", "type": "personal_chat", "id": 777, "messages": [
          {"id": 1, "type": "message", "date": "2024-01-01T10:00:00", "date_unixtime": "1704103200", "from": "Sam", "from_id": "user4242",
           "text": ["call me on ", {"type": "phone", "text": "+31 6 12345678"}, " tonight"]},
          {"id": 2, "type": "message", "date": "2024-01-01T10:01:00", "date_unixtime": "1704103260", "from": "Mara", "from_id": "user9",
           "text": "my email is mara.private@example.org"},
          {"id": 3, "type": "service", "date": "2024-01-01T10:02:00", "actor_id": "user4242", "action": "phone_call", "text": ""}
        ]},
        {"name": "Builders", "type": "private_supergroup", "id": 1500100200, "messages": [
          {"id": 55, "type": "message", "date": "2024-02-02T12:00:00", "date_unixtime": "1706875200", "from_id": "user4242",
           "text": "my password: Hunter2forreal", "photo": "photos/photo_1.jpg"}
        ]}
      ]}
    }"#;
    write_zip(path, &[("Telegram Desktop/DataExport_2026-11-01/result.json", json.into())]);
}

pub fn x_archive(path: &Path) {
    write_zip(
        path,
        &[
            ("data/account.js", r#"window.YTD.account.part0 = [{"account": {"accountId": "100", "username": "samtest"}}]"#.into()),
            (
                "data/tweets.js",
                r#"window.YTD.tweets.part0 = [
                  {"tweet": {"id_str": "1500000000000000001", "full_text": "new job! email me at sam.work@example.org", "created_at": "Thu Mar 04 05:06:07 +0000 2021"}},
                  {"tweet": {"id_str": "1500000000000000002", "full_text": "RT @someone: call 06-11112222", "created_at": "Thu Mar 04 06:06:07 +0000 2021"}}
                ]"#
                .into(),
            ),
            (
                "data/direct-messages.js",
                r#"window.YTD.direct_messages.part0 = [{"dmConversation": {"conversationId": "100-200", "messages": [
                  {"messageCreate": {"id": "9001", "senderId": "100", "recipientId": "200", "text": "my address is Prinsengracht 263", "createdAt": "2022-01-01T00:00:00.000Z"}},
                  {"messageCreate": {"id": "9002", "senderId": "200", "recipientId": "100", "text": "mine is Damrak 1", "createdAt": "2022-01-01T00:01:00.000Z"}}
                ]}}]"#
                .into(),
            ),
            ("data/tweetdeck.js", "window.YTD.tweetdeck.part0 = []".into()),
        ],
    );
}
