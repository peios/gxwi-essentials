//! The pages about change: the updates waiting, and what was done before.

use libgxwi::settings::{self, Glyph, Kind, Tile, Tone};

use crate::peipkg::{Operation, Transaction};
use crate::store::{Store, Updates};
use crate::words;

fn update_row(op: &Operation, may: bool) -> String {
    let from = op.from.as_deref().unwrap_or_default();
    let to = op.to.as_deref().unwrap_or_default();
    let line = format!("{from} → {to}");
    let control = settings::button("Update", "update", &[("name", &op.name)], Kind::Plain, may);
    settings::item(&settings::icon(Glyph::Refresh, Tile::Green), words::short(&op.name), &[(&line, false), (&op.name, true)], &control)
}

/// The updates waiting, as the last check found them.
pub fn updates(store: &Store, updates: &Updates, busy: bool) -> String {
    let mut page = settings::head(Glyph::Refresh, Tile::Green, "Updates", "Newer versions of what's installed, from the repositories.");
    let (banner, hidden) = store.banner();
    page.push_str(&banner);
    if hidden {
        return page;
    }
    let may = store.may.is_ok() && !busy;
    let again = |label: &str| settings::button(label, "check", &[], Kind::Plain, !busy && !matches!(updates, Updates::Checking));
    match updates {
        Updates::Unchecked | Updates::Checking => {
            page.push_str(&settings::hero(&settings::big("Checking", "Asking the repositories for anything newer…"), ""));
            page.push_str(&settings::group("", &settings::progress(0, 0, "Refreshing what the repositories offer…"), ""));
        }
        Updates::Found(ops) if ops.is_empty() => {
            page.push_str(&settings::hero(
                &settings::big("Up to Date", "Everything installed is the newest its repository offers."),
                &again("Check Again"),
            ));
        }
        Updates::Found(ops) => {
            let all = settings::button("Update All", "update-all", &[], Kind::Primary, may);
            page.push_str(&settings::hero(
                &settings::big(&ops.len().to_string(), if ops.len() == 1 { "update available" } else { "updates available" }),
                &format!("{}{all}", again("Check Again")),
            ));
            let rows: String = ops.iter().map(|op| update_row(op, may)).collect();
            let download: i64 = ops.iter().map(|o| o.size_download).sum();
            page.push_str(&settings::group("", &rows, &settings::hint(&format!("Downloads {} in all.", words::size(download)))));
        }
        Updates::Failed { code, message } if code == "stale" => {
            page.push_str(&settings::hero(&settings::big("Not Checked", "A repository's information is out of date."), &settings::pill("Out of Date", Tone::Warn)));
            let detail = words::detail(message);
            page.push_str(&settings::group(
                "",
                &settings::row(
                    "Refreshing it didn't bring anything newer",
                    &format!("{} You can check against what this machine already has. Checking anyway is recorded.", words::sentence(detail)),
                    &settings::button("Check Anyway", "check-anyway", &[], Kind::Plain, !busy),
                ),
                "",
            ));
        }
        Updates::Failed { code, .. } if code == "untrusted" => {
            page.push_str(&settings::hero(&settings::big("Not Checked", "A repository isn't trusted yet."), &settings::pill("Not Trusted", Tone::Bad)));
            page.push_str(&settings::group(
                "",
                &settings::row(
                    "Its trust hasn't been confirmed",
                    "It's set up on this machine, but nothing it offers is used until its trust is confirmed. Confirm it, or remove it, under Repositories.",
                    &settings::button("Repositories", "section", &[("section", "repositories")], Kind::Plain, true),
                ),
                "",
            ));
        }
        Updates::Failed { message, .. } => {
            page.push_str(&settings::hero(&settings::big("Not Checked", "The check for updates failed."), &again("Try Again")));
            page.push_str(&settings::group("", &settings::row("Why", &words::sentence(words::detail(message)), ""), ""));
        }
    }
    page
}

fn transaction_row(t: &Transaction, undo: bool, may: bool) -> String {
    // A change that went through is marked by what it did, where it did
    // one thing.
    let action = t.operations.first().map(|o| o.action.as_str()).filter(|a| t.operations.iter().all(|o| o.action == *a));
    let (glyph, tile, state) = match (t.state.as_str(), action) {
        ("committed", Some("install")) => (Glyph::Download, Tile::Blue, None),
        ("committed", Some("remove")) => (Glyph::Blocked, Tile::Red, None),
        ("committed", Some("upgrade")) => (Glyph::Refresh, Tile::Green, None),
        ("committed", Some("downgrade")) => (Glyph::Refresh, Tile::Orange, None),
        ("committed", _) => (Glyph::Package, Tile::Blue, None),
        // It failed, and peipkg put back what it had begun.
        ("rolled-back", _) => (Glyph::Info, Tile::Slate, Some(settings::pill("Didn't Finish", Tone::Plain))),
        _ => (Glyph::Info, Tile::Orange, Some(settings::pill("Interrupted", Tone::Bad))),
    };
    let mut control = state.unwrap_or_default();
    if undo {
        control.push_str(&settings::button("Undo…", "undo", &[], Kind::Plain, may));
    }
    let when = words::when(&t.started_at);
    settings::item(&settings::icon(glyph, tile), &what_was_done(t), &[(&when, false)], &control)
}

/// What a transaction did, in words: "make and cpio installed", "net
/// updated to 0.1.7-1".
fn what_was_done(t: &Transaction) -> String {
    let ops = &t.operations;
    let Some(first) = ops.first() else {
        return if t.summary.is_empty() { "A change".into() } else { words::sentence(&t.summary) };
    };
    let verb = |action: &str| match action {
        "install" => "installed",
        "upgrade" => "updated",
        "downgrade" => "moved back",
        "remove" => "removed",
        _ => "changed",
    };
    if let [only] = ops.as_slice() {
        let name = words::short(&only.name);
        return match only.action.as_str() {
            "upgrade" | "downgrade" if !only.to.is_empty() => format!("{name} {} to {}", verb(&only.action), only.to),
            action => format!("{name} {}", verb(action)),
        };
    }
    let names: Vec<&str> = ops.iter().map(|o| words::short(&o.name)).collect();
    let listed = match names.as_slice() {
        [a, b] => format!("{a} and {b}"),
        [a, b, c] => format!("{a}, {b} and {c}"),
        [a, b, rest @ ..] => format!("{a}, {b} and {} more", rest.len()),
        _ => names.join(", "),
    };
    if ops.iter().all(|o| o.action == first.action) {
        format!("{listed} {}", verb(&first.action))
    } else {
        format!("{listed} changed")
    }
}

/// Every change made, newest first.
pub fn history(store: &Store, busy: bool) -> String {
    let mut page = settings::head(Glyph::Clock, Tile::Slate, "History", "Every change made to this machine's software, newest first.");
    let (banner, hidden) = store.banner();
    page.push_str(&banner);
    if hidden {
        return page;
    }
    let may = store.may.is_ok() && !busy;
    let list = match &store.history {
        Ok(list) => list,
        Err(why) => return page + &settings::group("", &settings::row("The history couldn't be read", why, ""), ""),
    };
    if store.pending() {
        page.push_str(&settings::group(
            "",
            &settings::row(
                "A change was interrupted",
                "Nothing else can be changed until it's resolved. Resolving it puts back what it had begun.",
                &settings::button("Resolve", "recover", &[], Kind::Primary, may),
            ),
            "",
        ));
    }
    if list.is_empty() {
        return page
            + &settings::group("", &settings::row("No changes yet", "Changes made here, or with peipkg, are listed as they're made.", ""), "");
    }
    let undoable = store.undoable();
    let rows: String = list.iter().map(|t| transaction_row(t, Some(t.id) == undoable, may)).collect();
    let foot = settings::hint("Undo reverses the most recent change by making a new one, which is shown before it's made.");
    page + &settings::group("", &rows, &foot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peipkg::Done;

    fn done(action: &str, name: &str, to: &str) -> Done {
        Done { action: action.into(), name: name.into(), from: String::new(), to: to.into() }
    }

    #[test]
    fn a_change_is_said_by_what_it_did() {
        let t = |operations| Transaction { operations, summary: "x".into(), ..Default::default() };
        assert_eq!(what_was_done(&t(vec![done("install", "org.gnu.make", "4.4.1-1")])), "make installed");
        assert_eq!(what_was_done(&t(vec![done("upgrade", "dev.peios.net", "0.1.7-1")])), "net updated to 0.1.7-1");
        assert_eq!(what_was_done(&t(vec![done("install", "a.b", ""), done("install", "c.d", "")])), "b and d installed");
        let four = vec![done("remove", "a.p", ""), done("remove", "a.q", ""), done("remove", "a.r", ""), done("remove", "a.s", "")];
        assert_eq!(what_was_done(&t(four)), "p, q and 2 more removed");
        assert_eq!(what_was_done(&t(vec![done("install", "a.b", ""), done("remove", "c.d", "")])), "b and d changed");
        assert_eq!(what_was_done(&t(vec![])), "X.");
    }
}
