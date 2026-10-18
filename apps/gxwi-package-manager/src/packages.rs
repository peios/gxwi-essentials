//! The software pages: what is installed, what the repositories offer, and
//! one package.

use libgxwi::settings::{self, Glyph, Kind, Tile, Tone};
use libgxwi::{Fields, escape};

use crate::peipkg::{File, Offer, Package};
use crate::store::{Store, Updates};
use crate::words;

/// A package's mark in a list, drawn quietly.
fn badge(installed: bool) -> String {
    settings::icon(Glyph::Package, if installed { Tile::Blue } else { Tile::Teal })
}

/// The newer version an update check found for `name`, if it found one.
pub fn update_for<'a>(updates: &'a Updates, name: &str) -> Option<&'a str> {
    match updates {
        Updates::Found(ops) => ops.iter().find(|o| o.name == name).and_then(|o| o.to.as_deref()),
        _ => None,
    }
}

/// Whether `words` (lower case) are in a package's name or description.
fn matches(words: &str, name: &str, description: &str) -> bool {
    words.is_empty() || name.to_lowercase().contains(words) || description.to_lowercase().contains(words)
}

/// What a list says when peipkg couldn't be asked, or nothing matched.
fn none(text: &str, why: &str) -> String {
    settings::group("", &settings::row(text, why, ""), "")
}

fn installed_row(p: &Package, updates: &Updates) -> String {
    let line = if p.description.is_empty() { p.name.as_str() } else { p.description.as_str() };
    let mut aside = String::new();
    if update_for(updates, &p.name).is_some() {
        aside.push_str(&settings::pill("Update", Tone::Warn));
    } else if p.orphaned {
        aside.push_str(&settings::pill("No Repository", Tone::Plain));
    }
    aside.push_str(&format!(r#"<span class="version">{}</span>"#, escape(&p.version)));
    settings::link(&badge(true), words::short(&p.name), &[(line, false)], &aside, "open", &[("name", &p.name)])
}

/// Everything installed, narrowed to what the search finds.
pub fn installed(store: &Store, fields: &Fields, updates: &Updates) -> String {
    let mut page = settings::head(Glyph::Package, Tile::Blue, "Installed", "The software on this machine, and where each piece came from.");
    let (banner, hidden) = store.banner();
    page.push_str(&banner);
    if hidden {
        return page;
    }
    let list = match &store.installed {
        Ok(list) => list,
        Err(why) => return page + &none("The installed software couldn't be read", why),
    };
    let size: i64 = list.iter().map(|p| p.size_installed).sum();
    let right = match updates {
        Updates::Found(ops) if ops.is_empty() => settings::pill("Up to Date", Tone::Good),
        Updates::Found(ops) => {
            settings::button(&words::count(ops.len(), "Update", "Updates"), "section", &[("section", "updates")], Kind::Plain, true)
        }
        _ => String::new(),
    };
    page.push_str(&settings::hero(&settings::big(&list.len().to_string(), &format!("packages installed, taking {}", words::size(size))), &right));
    let orphans = list.iter().filter(|p| p.orphaned).count();
    if orphans > 0 {
        page.push_str(&settings::caution(&format!(
            "{} came from a repository this machine no longer uses. They stay installed, but nothing updates them.",
            words::count(orphans, "package", "packages")
        )));
    }
    page.push_str(&settings::search("search-installed", "Search installed software", "Search by name or description", ""));
    let typed = fields.get("search-installed").trim().to_lowercase();
    let rows: String = list.iter().filter(|p| matches(&typed, &p.name, &p.description)).map(|p| installed_row(p, updates)).collect();
    if rows.is_empty() {
        return page + &none(&format!("Nothing installed matches “{}”", fields.get("search-installed").trim()), "");
    }
    page + &settings::group("", &rows, "")
}

fn offer_row(o: &Offer) -> String {
    let line = if o.description.is_empty() { o.name.as_str() } else { o.description.as_str() };
    let aside = format!(r#"<span class="version">{}</span>"#, escape(&words::size(o.size_download)));
    settings::link(&badge(false), words::short(&o.name), &[(line, false)], &aside, "open", &[("name", &o.name)])
}

/// What the repositories offer that isn't installed.
pub fn available(store: &Store, fields: &Fields, choosing: bool) -> String {
    let mut page = settings::head(Glyph::Download, Tile::Teal, "Available", "Software the repositories offer that isn't installed yet.");
    let (banner, hidden) = store.banner();
    page.push_str(&banner);
    if hidden {
        return page;
    }
    if let Err(why) = &store.offers {
        return page + &none("The repositories' software couldn't be read", why);
    }
    let from_file =
        if store.may.is_ok() { settings::button("Install from File…", "install-file", &[], Kind::Plain, !choosing) } else { String::new() };
    page.push_str(&settings::search("search-available", "Search available software", "Search by name or description", &from_file));
    if store.repositories.as_ref().is_ok_and(Vec::is_empty) {
        return page
            + &settings::group(
                "",
                &settings::row(
                    "No repositories are set up",
                    "Software is installed from repositories. Add one to see what it offers.",
                    &settings::button("Repositories", "section", &[("section", "repositories")], Kind::Plain, true),
                ),
                "",
            );
    }
    let typed = fields.get("search-available").trim().to_lowercase();
    let offers = store.not_installed();
    let rows: String = offers.iter().filter(|o| matches(&typed, &o.name, &o.description)).map(|o| offer_row(o)).collect();
    if offers.is_empty() {
        return page + &none("Everything the repositories offer is installed", "");
    }
    if rows.is_empty() {
        return page + &none(&format!("Nothing available matches “{}”", fields.get("search-available").trim()), "");
    }
    let foot = settings::hint(&format!("{} from {}.", words::count(offers.len(), "package", "packages"), repositories(store)));
    page + &settings::group("", &rows, &foot)
}

/// The repositories, in words: "peios-dev and peios-medium".
fn repositories(store: &Store) -> String {
    let names: Vec<&str> = store.repositories.as_ref().map(|r| r.iter().map(|r| r.name.as_str()).collect()).unwrap_or_default();
    match names.as_slice() {
        [] => "no repository".into(),
        [one] => (*one).into(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What the page about one package needs that isn't in the store.
pub struct Looking<'a> {
    pub updates: &'a Updates,
    pub files: Option<&'a Result<Vec<File>, String>>,
    /// Where it was opened from, for the way back.
    pub from: &'static str,
    pub busy: bool,
}

/// One package: what it is, and what can be done with it.
pub fn detail(name: &str, store: &Store, looking: &Looking) -> String {
    let installed = store.package(name);
    let offer = store.offer(name);
    let description = installed.map(|p| p.description.as_str()).filter(|d| !d.is_empty()).or(offer.map(|o| o.description.as_str())).unwrap_or("");
    let update = update_for(looking.updates, name).or_else(|| {
        // Before a check, an offer newer than what's installed is one.
        let (p, o) = (installed?, offer?);
        words::newer(&o.version, &p.version).then_some(o.version.as_str())
    });
    let may = store.may.is_ok() && !looking.busy;

    let mut page = settings::back(looking.from, "back");
    let pill = match (installed, update) {
        (Some(_), Some(_)) => settings::pill("Update Available", Tone::Warn),
        (Some(_), None) => settings::pill("Installed", Tone::Good),
        (None, _) => settings::pill("Not Installed", Tone::Plain),
    };
    page.push_str(&settings::hero(&settings::hero_title(Glyph::Package, Tile::Blue, words::short(name), if description.is_empty() { name } else { description }), &pill));

    let mut buttons = String::new();
    match installed {
        Some(_) => {
            if let Some(to) = update {
                buttons.push_str(&settings::button(&format!("Update to {to}"), "update", &[("name", name)], Kind::Primary, may));
            }
            buttons.push_str(&settings::button("Check Files", "verify", &[("name", name)], Kind::Plain, !looking.busy));
            buttons.push_str(&settings::button("Remove…", "remove", &[("name", name)], Kind::Danger, may));
        }
        None if offer.is_some() => buttons.push_str(&settings::button("Install", "install", &[("name", name)], Kind::Primary, may)),
        None => {}
    }
    if !buttons.is_empty() {
        page.push_str(&settings::actions(&buttons));
    }
    if let Some(p) = installed
        && !p.alternate_upgrade.is_empty()
    {
        page.push_str(&settings::caution(&format!("Its publisher updates it another way: {}", p.alternate_upgrade)));
    }

    let mut facts = settings::fact("Package", name, true);
    match (installed, offer) {
        (Some(p), _) => {
            facts.push_str(&settings::fact("Version", &p.version, false));
            let source = match (p.origin.as_str(), p.orphaned) {
                ("", _) => "A file, not a repository".to_string(),
                (origin, true) => format!("{origin}, which this machine no longer uses"),
                (origin, false) => origin.to_string(),
            };
            facts.push_str(&settings::fact("Repository", &source, false));
            facts.push_str(&settings::fact("Installed", &words::date(&p.installed_at), false));
            facts.push_str(&settings::fact("Size", &words::size(p.size_installed), false));
            if !p.license.is_empty() {
                facts.push_str(&settings::fact("Licence", &p.license, false));
            }
            if !p.homepage.is_empty() {
                facts.push_str(&settings::fact("Website", &p.homepage, false));
            }
        }
        (None, Some(o)) => {
            facts.push_str(&settings::fact("Version", &o.version, false));
            facts.push_str(&settings::fact("Repository", &o.repository, false));
            facts.push_str(&settings::fact("Download", &words::size(o.size_download), false));
            facts.push_str(&settings::fact("Size Installed", &words::size(o.size_installed), false));
            if !o.license.is_empty() {
                facts.push_str(&settings::fact("Licence", &o.license, false));
            }
            if !o.homepage.is_empty() {
                facts.push_str(&settings::fact("Website", &o.homepage, false));
            }
        }
        (None, None) => return page + &none("This package is no longer installed or offered", ""),
    }
    page.push_str(&settings::group("Details", &facts, ""));

    if let Some(p) = installed {
        if !p.dependencies.is_empty() {
            let rows: String = p.dependencies.iter().map(|d| settings::fact(words::short(d.split(' ').next().unwrap_or(d)), d, true)).collect();
            page.push_str(&settings::group("Needs", &rows, ""));
        }
        if !p.provides.is_empty() {
            let rows: String = p.provides.iter().map(|d| settings::fact(d, "", false)).collect();
            page.push_str(&settings::group("Provides", &rows, ""));
        }
        page.push_str(&files(name, looking.files));
    }
    page
}

/// The files a package owns, once asked for: there can be thousands.
fn files(name: &str, files: Option<&Result<Vec<File>, String>>) -> String {
    const SHOWN: usize = 400;
    let Some(files) = files else {
        return settings::group(
            "Files",
            &settings::row("What it installed", "Every file, folder and link it put on this machine.", &settings::button("Show Files", "files", &[("name", name)], Kind::Plain, true)),
            "",
        );
    };
    let files = match files {
        Ok(files) => files,
        Err(why) => return settings::group("Files", &settings::row("The files couldn't be read", why, ""), ""),
    };
    let rows: String = files
        .iter()
        .filter(|f| f.kind != "dir")
        .take(SHOWN)
        .map(|f| {
            let path = if f.kind == "symlink" { format!("{} → {}", f.path, f.target) } else { f.path.clone() };
            format!(r#"<div class="st-row file"><code class="st-mono">{}</code></div>"#, escape(&path))
        })
        .collect();
    let shown = files.iter().filter(|f| f.kind != "dir").count();
    let foot = if shown > SHOWN { settings::hint(&format!("The first {SHOWN} of {shown}. peipkg files {name} lists them all.")) } else { String::new() };
    settings::group(&format!("Files ({shown})"), &rows, &foot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_matches_a_name_or_a_description() {
        assert!(matches("make", "org.gnu.make", ""));
        assert!(matches("build", "org.gnu.make", "Build automation"));
        assert!(!matches("zstd", "org.gnu.make", "Build automation"));
        assert!(matches("", "anything", ""));
    }
}
