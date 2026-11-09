//! A real server and real clients over QUIC on this machine.
use sompio_voice::client::{connect, Connect, Event, Outcome, Session};
use sompio_voice::proto::*;
use sompio_voice::server::{start, Options};
use std::time::Duration;
use tokio::sync::mpsc;

async fn next<F: Fn(&Event) -> bool>(rx: &mut mpsc::UnboundedReceiver<Event>, f: F) -> Event {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let e = tokio::time::timeout_at(deadline, rx.recv()).await.expect("timed out waiting").expect("closed");
        if f(&e) {
            return e;
        }
    }
}

async fn join(addr: &str, name: &str, seed: [u8; 32], fp: &str) -> (Session, mpsc::UnboundedReceiver<Event>, u32) {
    let (tx, rx) = mpsc::unbounded_channel();
    let Outcome::Connected(s) = connect(Connect { address: addr, name, password: None, master_identity: &seed, trusted: Some(fp) }, tx).await.unwrap() else { panic!("not connected") };
    let ServerMsg::Welcome { you, .. } = s.welcome.clone() else { panic!() };
    (s, rx, you)
}

#[tokio::test(flavor = "multi_thread")]
async fn channels_chat_voice_roles_and_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let r = start(Options { data_dir: dir.path().into(), port: 0, name: "Test server".into(), public: false }).await.unwrap();
    let addr = format!("127.0.0.1:{}", r.addr.port());
    let admin_key = r.first_admin_key.clone().unwrap();

    // First contact: we're shown the fingerprint before anything is sent
    let (tx, _rx) = mpsc::unbounded_channel();
    match connect(Connect { address: &addr, name: "Alex", password: None, master_identity: &[1; 32], trusted: None }, tx).await.unwrap() {
        Outcome::NewServer { fingerprint, name } => {
            assert_eq!(fingerprint, r.fingerprint);
            assert_eq!(name, "Test server");
        }
        _ => panic!("expected a new-server prompt"),
    }
    // A different pinned fingerprint is refused
    let (tx, _rx) = mpsc::unbounded_channel();
    assert!(matches!(
        connect(Connect { address: &addr, name: "Alex", password: None, master_identity: &[1; 32], trusted: Some("AAAA:BBBB") }, tx).await.unwrap(),
        Outcome::Changed { .. }
    ));

    let fp = r.fingerprint.clone();
    let (a, mut arx, a_id) = join(&addr, "Alex", [1; 32], &fp).await;
    let (b, mut brx, b_id) = join(&addr, "Sam", [2; 32], &fp).await;
    next(&mut arx, |e| matches!(e, Event::Msg(ServerMsg::ClientJoined { client }) if client.name == "Sam")).await;

    // Same name gets a number
    let (c, _crx, _) = join(&addr, "sam", [3; 32], &fp).await;
    next(&mut arx, |e| matches!(e, Event::Msg(ServerMsg::ClientJoined { client }) if client.name == "sam (2)")).await;
    c.close().await;

    // Not an admin yet
    a.send(ClientMsg::CreateChannel { name: "Raid".into(), parent: None, topic: "".into(), password: Some("owls".into()), permanent: true, max_users: 0 }).unwrap();
    next(&mut arx, |e| matches!(e, Event::Msg(ServerMsg::Error { .. }))).await;
    a.send(ClientMsg::UsePrivilegeKey { key: admin_key.to_lowercase() }).unwrap();
    next(&mut arx, |e| matches!(e, Event::Msg(ServerMsg::ClientUpdated { client }) if client.id == a_id && client.role == Role::Admin)).await;
    a.send(ClientMsg::CreateChannel { name: "Raid".into(), parent: None, topic: "Wednesdays".into(), password: Some("owls".into()), permanent: true, max_users: 0 }).unwrap();
    let raid = match next(&mut brx, |e| matches!(e, Event::Msg(ServerMsg::ChannelSaved { .. }))).await {
        Event::Msg(ServerMsg::ChannelSaved { channel }) => channel,
        _ => unreachable!(),
    };
    assert!(raid.has_password);

    // Chat in the channel: Alex moved into Raid on creating it, Sam is in the Lobby
    a.send(ClientMsg::Chat { target: ChatTarget::Server, text: "hello everyone".into() }).unwrap();
    next(&mut brx, |e| matches!(e, Event::Msg(ServerMsg::Chat { text, .. }) if text == "hello everyone")).await;

    // Passwords on channels
    b.send(ClientMsg::Join { channel: raid.id, password: None }).unwrap();
    next(&mut brx, |e| matches!(e, Event::Msg(ServerMsg::Error { message }) if message.contains("password"))).await;
    b.send(ClientMsg::Join { channel: raid.id, password: Some("owls".into()) }).unwrap();
    next(&mut arx, |e| matches!(e, Event::Msg(ServerMsg::ClientUpdated { client }) if client.id == b_id && client.channel == raid.id)).await;

    // Voice reaches people in the same channel
    tokio::time::sleep(Duration::from_millis(100)).await;
    for seq in 0..5u16 {
        a.send_voice(seq, b"opus-frame");
    }
    match next(&mut brx, |e| matches!(e, Event::Voice { .. })).await {
        Event::Voice { from, data, .. } => {
            assert_eq!(from, a_id);
            assert_eq!(data, b"opus-frame");
        }
        _ => unreachable!(),
    }
    // ...but not someone who's deafened
    b.send(ClientMsg::SetState { muted: false, deafened: true, away: None }).unwrap();
    next(&mut arx, |e| matches!(e, Event::Msg(ServerMsg::ClientUpdated { client }) if client.deafened)).await;
    while brx.try_recv().is_ok() {}
    a.send_voice(99, b"not for sam");
    tokio::time::sleep(Duration::from_millis(300)).await;
    while let Ok(e) = brx.try_recv() {
        assert!(!matches!(e, Event::Voice { .. }), "deafened people get no voice");
    }

    // Kick from the server
    a.send(ClientMsg::Kick { client: b_id, reason: "test".into(), from_server: true }).unwrap();
    match next(&mut brx, |e| matches!(e, Event::Closed { .. })).await {
        Event::Closed { reason } => assert!(reason.contains("test"), "{reason}"),
        _ => unreachable!(),
    }
    a.close().await;
    r.stop().await;

    // Restart: same certificate, the permanent channel and Alex's admin role are kept
    let r2 = start(Options { data_dir: dir.path().into(), port: 0, name: "ignored".into(), public: false }).await.unwrap();
    assert_eq!(r2.fingerprint, fp);
    assert!(r2.first_admin_key.is_none());
    let addr2 = format!("127.0.0.1:{}", r2.addr.port());
    let (a2, _rx, _) = join(&addr2, "Alex", [1; 32], &fp).await;
    let ServerMsg::Welcome { role, channels, server, .. } = a2.welcome.clone() else { panic!() };
    assert_eq!(role, Role::Admin, "same person, same role");
    assert_eq!(server.name, "Test server");
    assert!(channels.iter().any(|c| c.name == "Raid"));
    a2.close().await;
    r2.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn passwords_bans_and_temporary_channels() {
    let dir = tempfile::tempdir().unwrap();
    let r = start(Options { data_dir: dir.path().into(), port: 0, name: "Private".into(), public: false }).await.unwrap();
    let key = r.first_admin_key.clone().unwrap();
    let fp = r.fingerprint.clone();
    r.stop().await;
    sompio_voice::server::set_password(dir.path(), Some("letmein")).unwrap();
    let r = start(Options { data_dir: dir.path().into(), port: 0, name: "".into(), public: false }).await.unwrap();
    let addr = format!("127.0.0.1:{}", r.addr.port());
    let (tx, _rx) = mpsc::unbounded_channel();
    let err = connect(Connect { address: &addr, name: "X", password: None, master_identity: &[5; 32], trusted: Some(&fp) }, tx).await.err().unwrap();
    assert!(err.to_string().contains("password"));
    let (tx, _rx) = mpsc::unbounded_channel();
    let err = connect(Connect { address: &addr, name: "X", password: Some("nope"), master_identity: &[5; 32], trusted: Some(&fp) }, tx).await.err().unwrap();
    assert!(err.to_string().contains("Wrong server password"), "{err}");

    let open = |seed: [u8; 32], name: &'static str| {
        let addr = addr.clone();
        let fp = fp.clone();
        async move {
            let (tx, rx) = mpsc::unbounded_channel();
            let Outcome::Connected(s) = connect(Connect { address: &addr, name, password: Some("letmein"), master_identity: &seed, trusted: Some(&fp) }, tx).await.unwrap() else { panic!() };
            let ServerMsg::Welcome { you, .. } = s.welcome.clone() else { panic!() };
            (s, rx, you)
        }
    };
    let (admin, mut arx, _) = open([1; 32], "Admin").await;
    admin.send(ClientMsg::UsePrivilegeKey { key }).unwrap();
    next(&mut arx, |e| matches!(e, Event::Msg(ServerMsg::ClientUpdated { client }) if client.role == Role::Admin)).await;
    let (troll, mut trx, troll_id) = open([6; 32], "Troll").await;

    // Anyone can make a temporary channel; it goes when empty
    troll.send(ClientMsg::CreateChannel { name: "Temp".into(), parent: None, topic: "".into(), password: None, permanent: false, max_users: 0 }).unwrap();
    let temp = match next(&mut arx, |e| matches!(e, Event::Msg(ServerMsg::ChannelSaved { .. }))).await {
        Event::Msg(ServerMsg::ChannelSaved { channel }) => channel.id,
        _ => unreachable!(),
    };
    troll.send(ClientMsg::Join { channel: 1, password: None }).unwrap();
    next(&mut arx, |e| matches!(e, Event::Msg(ServerMsg::ChannelDeleted { id }) if *id == temp)).await;

    admin.send(ClientMsg::Ban { client: troll_id, reason: "spam".into(), minutes: 60 }).unwrap();
    next(&mut trx, |e| matches!(e, Event::Closed { .. })).await;
    drop(troll);
    let (tx, _rx) = mpsc::unbounded_channel();
    let err = connect(Connect { address: &addr, name: "Troll", password: Some("letmein"), master_identity: &[6; 32], trusted: Some(&fp) }, tx).await.err().unwrap();
    assert!(err.to_string().contains("banned"), "{err}");
    admin.close().await;
    r.stop().await;
}
