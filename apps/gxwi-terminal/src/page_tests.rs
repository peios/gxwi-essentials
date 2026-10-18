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
