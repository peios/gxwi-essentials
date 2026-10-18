use super::*;

#[test]
fn a_page_asks_for_a_size_and_for_nothing_else() {
    assert_eq!(
        asked(r#"{"type":"size","rows":30,"cols":100}"#),
        Some(Size {
            rows: 30,
            cols: 100
        })
    );
    assert_eq!(asked(r#"{"type":"size","rows":1e9,"cols":0}"#), None);
    assert_eq!(
        asked(r#"{"type":"size","rows":100000,"cols":0}"#),
        Some(Size { rows: 500, cols: 2 })
    );
    assert_eq!(asked(r#"{"type":"size","rows":-3,"cols":10}"#), None);
    assert_eq!(asked(r#"{"type":"ended"}"#), None);
    assert_eq!(asked("not json"), None);
}

#[test]
fn the_page_asks_for_nothing_that_is_not_served() {
    let page = include_str!("../page/index.html");
    let script = include_str!("../page/term.js");
    for asked in [
        "term.css",
        "term.js",
        "vendor/xterm.css",
        "../.fx/tokens.css",
        "../.fx/keys.js",
    ] {
        assert!(page.contains(&format!("\"{asked}\"")), "{asked}");
    }
    for asked in ["./vendor/xterm.mjs", "./vendor/addon-fit.mjs"] {
        assert!(script.contains(&format!("\"{asked}\"")), "{asked}");
    }
    // Nothing is asked for from the root: where the app's pages are is
    // not for the page to know.
    assert!(!page.contains("=\"/"));
}

#[test]
fn a_row_has_room_for_an_underscore() {
    // At a line height of 1, each row is the measured cell and cuts off
    // what is drawn below it, which in the monospace faces the desktop
    // uses is where an underscore is: `a_b_c` showed as `a b c`.
    let script = include_str!("../page/term.js");
    let height: f64 = script
        .split("lineHeight:")
        .nth(1)
        .and_then(|rest| rest.split(',').next())
        .and_then(|value| value.trim().parse().ok())
        .expect("term.js sets lineHeight");
    assert!(height >= 1.15, "lineHeight {height}");
    // And it opens on the font it draws with, not one that loads after.
    assert!(script.contains("await document.fonts.load("));
}
/// A subprocess fixture whose PTY deliberately never consumes input.
#[test]
fn unreading_child() {
    if std::env::var_os("GXWI_TEST_UNREADING").is_none() {
        return;
    }
    use std::io::Write as _;
    let mut mode = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: tcgetattr initializes the live termios storage on success.
    assert_eq!(unsafe { libc::tcgetattr(0, mode.as_mut_ptr()) }, 0);
    // SAFETY: the successful call above initialized every field.
    let mut mode = unsafe { mode.assume_init() };
    // SAFETY: mode is initialized and both calls retain no pointers.
    unsafe {
        libc::cfmakeraw(&mut mode);
        assert_eq!(libc::tcsetattr(0, libc::TCSANOW, &mode), 0);
    }
    std::io::stdout()
        .write_all(b"input-fixture-ready\n")
        .unwrap();
    std::io::stdout().flush().unwrap();
    let signal = std::env::var_os("GXWI_TEST_SIGNAL").unwrap();
    while !std::path::Path::new(&signal).exists() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    std::io::stdout()
        .write_all(b"output-while-input-blocked\n")
        .unwrap();
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

#[tokio::test]
async fn output_keeps_flowing_while_input_waits_and_child_exit_releases_it() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message as ClientMessage;

    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    struct Signal(std::path::PathBuf);
    impl Drop for Signal {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let signal =
        Signal(std::env::temp_dir().join(format!("gxwi-terminal-output-{}", std::process::id())));
    assert!(!signal.0.exists());
    let (pty, child) = crate::pty::Pty::start(
        &std::env::current_exe().unwrap(),
        &[
            "--exact".into(),
            "page::tests::unreading_child".into(),
            "--nocapture".into(),
        ],
        std::path::Path::new("/"),
        &[
            ("GXWI_TEST_UNREADING", "1"),
            ("GXWI_TEST_SIGNAL", signal.0.to_str().unwrap()),
        ],
        Size::USUAL,
    )
    .unwrap();
    let mut child = Child(child);
    let terminal = Terminal::new(pty, Size::USUAL).unwrap();
    let followed = Arc::clone(&terminal);
    let reader = std::thread::spawn(move || followed.follow(|_| {}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let served = Arc::clone(&terminal);
    let server = tokio::spawn(async move {
        axum::serve(listener, routes(served)).await.unwrap();
    });
    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/term/socket"))
        .await
        .unwrap();
    let deadline = std::time::Duration::from_secs(5);
    let mut output = Vec::new();
    while !String::from_utf8_lossy(&output).contains("input-fixture-ready") {
        let message = tokio::time::timeout(deadline, socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let ClientMessage::Binary(bytes) = message {
            output.extend_from_slice(&bytes);
        }
    }
    socket
        .send(ClientMessage::Binary(vec![b'x'; MAX_INPUT_BYTES].into()))
        .await
        .unwrap();
    tokio::time::timeout(deadline, async {
        while terminal.pending_input_bytes() != MAX_INPUT_BYTES {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    std::fs::write(&signal.0, b"produce output").unwrap();
    while !String::from_utf8_lossy(&output).contains("output-while-input-blocked") {
        let message = tokio::time::timeout(deadline, socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        match message {
            ClientMessage::Binary(bytes) => output.extend_from_slice(&bytes),
            ClientMessage::Text(text) => assert!(
                !text.contains("\"typed\""),
                "input unexpectedly drained: {text}"
            ),
            _ => {}
        }
    }
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    loop {
        let message = tokio::time::timeout(deadline, socket.next()).await.unwrap();
        match message {
            Some(Ok(ClientMessage::Text(text)))
                if text.contains("input-error") || text.contains("ended") =>
            {
                break;
            }
            None | Some(Err(_)) => break,
            _ => {}
        }
    }
    reader.join().unwrap();
    server.abort();
}
