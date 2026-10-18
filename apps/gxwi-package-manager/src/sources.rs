//! The pages about where software comes from and goes: repositories, the
//! roles packages fill, and the installation roots.

use libgxwi::Fields;
use libgxwi::settings::{self, Glyph, Kind, More, Tile, Tone, Width};

use crate::manager::Asking;
use crate::peipkg::{Repository, Root};
use crate::run::focused;
use crate::store::Store;
use crate::words;

fn repository_row(r: &Repository, asking: Option<&Asking>, may: bool, now: i64) -> String {
    let mut state = String::new();
    if !r.trusted {
        state.push_str(&settings::pill("Not Trusted", Tone::Bad));
    } else if r.stale {
        state.push_str(&settings::pill("Out of Date", Tone::Warn));
    }
    if r.signature_policy != "required" {
        state.push_str(&settings::pill("Unsigned Allowed", Tone::Warn));
    }
    let mut buttons = String::new();
    if !r.trusted {
        buttons.push_str(&settings::button("Trust…", "ask-trust-repo", &[("repo", &r.name)], Kind::Primary, may));
    }
    buttons.push_str(&settings::button("Remove…", "ask-remove-repo", &[("repo", &r.name)], Kind::Plain, may));
    let refreshed = match &r.last_refresh {
        Some(at) => format!("{} · refreshed {}", words::count(r.packages, "package", "packages"), words::ago(at, now)),
        None if r.trusted => "Never refreshed".to_string(),
        None => "Its trust hasn't been confirmed, so nothing it offers is used".to_string(),
    };
    let mut row = settings::item(
        &settings::icon(Glyph::Server, Tile::Violet),
        &r.name,
        &[(&r.base_url, true), (&refreshed, false)],
        &format!("{state}{buttons}"),
    );
    if asking == Some(&Asking::Trust(r.name.clone())) {
        let key = r.trust_anchors.first().map(|k| fingerprint(k)).unwrap_or_default();
        let keys = match r.trust_anchors.len() {
            0 => "No signing key is given for it, so its packages aren't checked.".to_string(),
            1 => "Its signing key is given as".to_string(),
            n => format!("{n} signing keys are given for it; the first is"),
        };
        row.push_str(&settings::more(
            More::Asking,
            &format!(
                "<p>Trust {}? It was set up on this machine without being confirmed. Trusting it fetches what it offers and checks that against the key given for it, then installs and updates from it.</p><p>{}</p>{}{}",
                libgxwi::escape(&r.name),
                libgxwi::escape(&keys),
                if key.is_empty() { String::new() } else { format!(r#"<p><code class="st-mono">{}</code></p>"#, libgxwi::escape(&key)) },
                settings::actions(&format!(
                    "{}{}",
                    focused("Cancel", "cancel", &[], Kind::Plain),
                    settings::button("Trust", "trust-repo", &[("repo", &r.name)], Kind::Primary, true)
                ))
            ),
        ));
    }
    if asking == Some(&Asking::Remove(r.name.clone())) {
        row.push_str(&settings::more(
            More::Asking,
            &format!(
                "<p>Remove {}? Software installed from it stays installed, but nothing updates it.</p>{}",
                libgxwi::escape(&r.name),
                settings::actions(&format!(
                    "{}{}",
                    focused("Cancel", "cancel", &[], Kind::Plain),
                    settings::button("Remove", "remove-repo", &[("repo", &r.name)], Kind::Danger, true)
                ))
            ),
        ));
    }
    row
}

/// A key's fingerprint in groups of four, as it is read out and compared.
fn fingerprint(hex: &str) -> String {
    hex.as_bytes().chunks(4).map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>().join(" ")
}

/// The form for a repository to add.
fn adding() -> String {
    let fields = format!(
        r#"<div class="fields stacked"><label>Name{}</label><label>Address{}</label><label>Signing Key Fingerprint{}</label></div><p>The publisher gives the address and the key's fingerprint. Get the fingerprint from them directly: it's what proves their packages are theirs.</p>{}"#,
        settings::text("repo-name", "Name", "text", Width::Normal, true, r#"placeholder="example" autocomplete="off" spellcheck="false" fx-autofocus"#),
        settings::text("repo-url", "Address", "text", Width::Wide, true, r#"placeholder="https://… or file:///…" autocomplete="off" spellcheck="false""#),
        settings::text("repo-key", "Signing key fingerprint", "text", Width::Wide, true, r#"placeholder="64 hexadecimal digits" autocomplete="off" spellcheck="false""#)
            .replacen("st-input", "st-input mono", 1),
        settings::actions(&format!("{}{}", settings::button("Cancel", "cancel", &[], Kind::Plain, true), settings::submit("Add Repository", Kind::Primary, true))),
    );
    settings::more(More::Form, &format!(r#"<form fx-submit="add-repo">{fields}</form>"#))
}

/// The repositories, and adding and removing them.
pub fn repositories(store: &Store, asking: Option<&Asking>, busy: bool, now: i64) -> String {
    let mut page = settings::head(Glyph::Server, Tile::Violet, "Repositories", "Where software comes from, and the keys that vouch for it.");
    let (banner, hidden) = store.banner();
    page.push_str(&banner);
    if hidden {
        return page;
    }
    let may = store.may.is_ok() && !busy;
    let list = match &store.repositories {
        Ok(list) => list,
        Err(why) => return page + &settings::group("", &settings::row("The repositories couldn't be read", why, ""), ""),
    };
    if store.may.is_ok() {
        page.push_str(&settings::actions(&format!(
            "{}{}",
            settings::button("Refresh All", "refresh", &[], Kind::Plain, may && list.iter().any(|r| r.trusted)),
            settings::button("Add Repository…", "ask-add-repo", &[], Kind::Primary, may && asking != Some(&Asking::Add))
        )));
    }
    let mut rows: String = list.iter().map(|r| repository_row(r, asking, may, now)).collect();
    if list.is_empty() {
        rows = settings::row("No repositories", "Without one, nothing can be installed or updated.", "");
    }
    if asking == Some(&Asking::Add) {
        rows.push_str(&adding());
    }
    let foot = settings::hint("A repository is trusted once its signing key's fingerprint is checked, when it's added. Out of date means its information is older than it may be trusted for.");
    page + &settings::group("", &rows, &foot)
}

/// The roles packages fill, and which one does.
pub fn roles(store: &Store, busy: bool) -> String {
    let mut page = settings::head(Glyph::Tag, Tile::Orange, "Roles", "Commands more than one package could provide, such as sh, and which one does.");
    let (banner, hidden) = store.banner();
    page.push_str(&banner);
    if hidden {
        return page;
    }
    let may = store.may.is_ok() && !busy;
    let list = match &store.roles {
        Ok(list) => list,
        Err(why) => return page + &settings::group("", &settings::row("The roles couldn't be read", why, ""), ""),
    };
    if list.is_empty() {
        return page + &settings::group("", &settings::row("No roles", "No installed package fills a role.", ""), "");
    }
    let rows: String = list
        .iter()
        .map(|r| {
            let links: Vec<String> = r.links.iter().map(|l| format!("{} → {}", l.path, l.target)).collect();
            let about = match &r.holder {
                Some(holder) if links.is_empty() => format!("Provided by {holder}"),
                Some(_) => links.join(", "),
                None => "Not provided: nothing answers to it.".to_string(),
            };
            let control = if r.providers.len() > 1 || (r.holder.is_none() && !r.providers.is_empty()) {
                let mut options: Vec<(String, String)> = r.providers.iter().map(|p| (p.clone(), words::short(p).to_string())).collect();
                if r.holder.is_none() {
                    options.insert(0, (String::new(), "None".into()));
                }
                settings::select(&format!("role:{}", r.role), &format!("What provides {}", r.role), &options, may)
            } else {
                settings::value(r.holder.as_deref().map(words::short).unwrap_or("None"), false)
            };
            settings::row(&r.role, &about, &control)
        })
        .collect();
    let foot = settings::hint("Choosing another package moves every link of the role to it at once, as one change you can undo.");
    page + &settings::group("", &rows, &foot)
}

/// Fills the role choices with who holds each.
pub fn fill_roles(store: &Store, fields: &mut Fields) {
    if let Ok(roles) = &store.roles {
        for r in roles {
            fields.set(&format!("role:{}", r.role), r.holder.as_deref().unwrap_or(""));
        }
    }
}

fn root_rows(roots: &[Root], within: &str, out: &mut String) {
    for r in roots {
        let name = if within.is_empty() { r.name.clone() } else { format!("{within} / {}", r.name) };
        let state = if r.status == "present" { settings::pill("Present", Tone::Good) } else { settings::pill("Missing", Tone::Bad) };
        out.push_str(&settings::item(&settings::icon(Glyph::Folder, Tile::Pink), &name, &[(&r.resolved_path, true)], &state));
        root_rows(&r.children, &name, out);
    }
}

/// The other trees this machine installs software into.
pub fn roots(store: &Store) -> String {
    let mut page = settings::head(
        Glyph::Folder,
        Tile::Pink,
        "Installation Roots",
        "Other folder trees this machine installs software into, such as its boot image, each with packages of its own.",
    );
    // Nothing here is changed, so only a reason not to see it is said.
    if let Err(why) = &store.look {
        page.push_str(&settings::banner(why));
        return page;
    }
    let list = match &store.roots {
        Ok(list) => list,
        Err(why) => return page + &settings::group("", &settings::row("The roots couldn't be read", why, ""), ""),
    };
    if list.is_empty() {
        return page + &settings::group("", &settings::row("No other roots", "Everything is installed into this machine's own tree.", ""), "");
    }
    let mut rows = String::new();
    root_rows(list, "", &mut rows);
    page + &settings::group("", &rows, &settings::hint("Updates reach every root. Roots are added and removed with peipkg root."))
}
