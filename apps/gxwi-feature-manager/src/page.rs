//! The page: each feature as a card of what it is, how it is set up, and
//! the changes that apply to it, with a change being made shown on its own
//! feature's card.

use libgxwi::escape;
use libgxwi::settings::{self, Glyph, Kind, More, Tile, Tone};

use crate::feat::{Feature, State};
use crate::manager::{Failure, Listing, Manager, Run, Verb};

const ABOUT: &str = "Features set this machine up beyond the files packages install: services, settings and what they need. Each comes in a package, and turning one on or off runs its own scripts, as you.";

const DENIED: &str = "You can't change features: that needs write access to their state in the registry. As shipped, only Administrators have it.";

pub fn page(m: &Manager) -> String {
    let mut page = settings::head(Glyph::Toggle, Tile::Teal, "Features", ABOUT);
    let features = match &m.features {
        Listing::Reading => return page + &settings::group("", &settings::row("Reading the features…", "", ""), ""),
        Listing::Failed(why) => return page + &settings::group("", &settings::row("The features can't be read", &sentence(why), ""), ""),
        Listing::Listed(features) if features.is_empty() => {
            return page
                + &settings::group(
                    "",
                    &settings::row("No features", "None is on this machine. A feature comes in a package: install one with Package Manager, and it is listed here.", ""),
                    "",
                );
        }
        Listing::Listed(features) => features,
    };
    if m.close_when_done && m.busy() {
        page.push_str(&settings::caution("Feature Manager closes when this change is finished."));
    }
    // Said once for the page where it is so of every feature.
    let none_may = !features.iter().any(|f| f.may_change);
    if none_may {
        page.push_str(&settings::banner(DENIED));
    }
    for feature in features {
        page.push_str(&card(m, feature, none_may));
    }
    page
}

/// How a feature is, as its tile's colour and a pill.
fn looks(f: &Feature) -> (Tile, String) {
    let (tile, text, tone) = if !f.defined {
        (Tile::Red, "Definition Missing", Tone::Bad)
    } else {
        match f.state {
            State::Enabled => (Tile::Green, "On", Tone::Good),
            State::Installed => (Tile::Blue, "Installed, Off", Tone::Plain),
            State::NotInstalled => (Tile::Slate, "Not Installed", Tone::Plain),
            _ => (Tile::Orange, "Interrupted", Tone::Warn),
        }
    };
    (tile, settings::pill(text, tone))
}

/// What a feature is, as paragraphs.
fn about(description: &str) -> String {
    let paragraphs: String = description.split("\n\n").map(|p| format!("<p>{}</p>", escape(p))).collect();
    format!(r#"<div class="st-row about">{paragraphs}</div>"#)
}

/// A word on the feature as a row of its card.
fn word(text: &str) -> String {
    format!(r#"<div class="st-row word"><span class="label"><small>{}</small></span></div>"#, escape(text))
}

/// Why a feature is left part-way, and what each way on does.
fn interrupted(state: State) -> &'static str {
    match state {
        State::Installing => "Installing it didn't finish: its install script failed or was stopped. Try Again runs it again; Remove takes away whatever it set up.",
        State::Enabling => "Turning it on didn't finish: its enable script failed or was stopped. Try Again runs it again; Turn Off puts it back off.",
        State::Disabling => "Turning it off didn't finish: its disable script failed or was stopped. Try Again runs it again; Turn On puts it back on.",
        _ => "Removing it didn't finish: its uninstall script failed or was stopped. Try Again runs it again; Install Again puts it back.",
    }
}

/// The changes that apply to a feature as it is: the lesser first, the one
/// its state asks for last.
fn changes(f: &Feature) -> Vec<(&'static str, Verb, Kind)> {
    match f.state {
        State::NotInstalled => vec![("Install Only", Verb::Install, Kind::Plain), ("Turn On", Verb::Add, Kind::Primary)],
        State::Installed => vec![("Remove…", Verb::Remove, Kind::Plain), ("Turn On", Verb::Enable, Kind::Primary)],
        State::Enabled => vec![("Remove…", Verb::Remove, Kind::Plain), ("Turn Off", Verb::Disable, Kind::Primary)],
        State::Installing => vec![("Remove…", Verb::Remove, Kind::Plain), ("Try Again", Verb::Install, Kind::Primary)],
        State::Enabling => vec![("Turn Off", Verb::Disable, Kind::Plain), ("Try Again", Verb::Enable, Kind::Primary)],
        State::Disabling => vec![("Turn On", Verb::Enable, Kind::Plain), ("Try Again", Verb::Disable, Kind::Primary)],
        State::Uninstalling => vec![("Install Again", Verb::Install, Kind::Plain), ("Try Again", Verb::Remove, Kind::Primary)],
    }
}

fn change(label: &str, verb: Verb, f: &Feature, kind: Kind, enabled: bool) -> String {
    // Removing asks first, under the card, unless it is finishing a removal.
    let (event, kind) = if label == "Remove…" { ("ask-remove", Kind::Plain) } else { ("change", kind) };
    settings::button(label, event, &[("verb", verb.arg()), ("name", &f.name)], kind, enabled)
}

/// The question before a removal.
fn asking_remove(f: &Feature) -> String {
    let off = if f.state == State::Enabled { "It is turned off, then its" } else { "Its" };
    settings::more(
        More::Asking,
        &format!(
            "<p>Remove {}? {off} uninstall script runs, taking away what it set up. The feature stays on this machine, ready to install again.</p>{}",
            escape(f.shown()),
            settings::actions(&format!(
                "{}{}",
                settings::focused_button("Cancel", "cancel", Kind::Plain),
                settings::button("Remove", "change", &[("verb", Verb::Remove.arg()), ("name", &f.name)], Kind::Danger, true),
            ))
        ),
    )
}

/// The last row of a card: what can be done to the feature.
fn bottom(html: &str) -> String {
    format!(r#"<div class="st-row bottom">{html}</div>"#)
}

/// A change being made to this feature, as a bar with the last thing said
/// under it.
fn going(run: &Run) -> String {
    let text = match &run.progress {
        Some((phase, step, steps)) if *steps > 1 => format!("Running its {phase} script ({step} of {steps})…"),
        Some((phase, _, _)) => format!("Running its {phase} script…"),
        None => format!("{}…", run.verb.doing()),
    };
    let last = run.output.last().map(|line| format!(r#"<small class="said">{}</small>"#, escape(line))).unwrap_or_default();
    // How long a script takes isn't known, so the bar doesn't say how far.
    format!(r#"<div class="st-row going">{}{last}</div>"#, settings::progress(0, 0, &text))
}

/// What a failure means, and whether running it again is the way on.
fn failure(run: &Run, failure: &Failure) -> (String, String, bool) {
    let phase = run.progress.as_ref().map_or("", |(phase, _, _)| phase.as_str());
    match failure.code.as_str() {
        "denied" => (
            "You can't change this feature".into(),
            "Changing a feature needs write access to its state in the registry. As shipped, only Administrators have it.".into(),
            false,
        ),
        "script" => (
            format!("Its {phase} script failed"),
            format!(
                "{} is left part-way, as Interrupted. What the script said is below. Once its cause is put right, Try Again runs it again.",
                run.shown
            ),
            true,
        ),
        "not-found" => (
            "This feature is gone".into(),
            "Its definition isn't on this machine any more: the package that provided it may have been removed.".into(),
            false,
        ),
        // feat's own word, without the command it suggests at a terminal.
        "state" => ("This can't be done now".into(), sentence(failure.message.split(" (run ").next().unwrap_or_default()), false),
        _ => ("The change failed".into(), sentence(&failure.message), true),
    }
}

/// A failed change to this feature: why, what was said, and the way on.
fn failed(run: &Run, f: &Failure) -> String {
    let (title, why, again) = failure(run, f);
    let log = if run.output.is_empty() { String::new() } else { format!("<pre>{}</pre>", escape(&run.output.join("\n"))) };
    let again = if again {
        settings::button("Try Again", "change", &[("verb", run.verb.arg()), ("name", &run.name)], Kind::Primary, true)
    } else {
        String::new()
    };
    settings::more(
        More::Asking,
        &format!(
            "<p><b>{}</b><br>{}</p>{log}{}",
            escape(&title),
            escape(&why),
            settings::actions(&format!("{}{again}", settings::focused_button("Close", "dismiss", Kind::Plain)))
        ),
    )
}

/// One feature, as a card.
fn card(m: &Manager, f: &Feature, banner: bool) -> String {
    let (tile, pill) = looks(f);
    let mut rows = settings::item(&settings::icon(Glyph::Toggle, tile), f.shown(), &[(f.name.as_str(), true)], &pill);
    if let Some(description) = &f.description {
        rows.push_str(&about(description));
    }
    if let Some(problem) = &f.metadata_problem {
        rows.push_str(&word(&format!("What it says of itself can't be read: {}", sentence(problem))));
    }
    if !f.defined {
        rows.push_str(&word(
            "Its definition isn't on this machine any more: the package that provided it was removed while it was set up, and what it set up is still in place. Installing the package that provided it again lets it be turned off or removed.",
        ));
        return settings::group("", &rows, "");
    }
    let run = m.run.as_ref().filter(|r| r.name == f.name);
    let failure = run.and_then(|r| r.failed.as_ref().map(|failure| (r, failure)));
    // A failure just shown says why it is left part-way, so the standing
    // word on it waits until the failure is put away.
    if f.state.interrupted() && failure.is_none() {
        rows.push_str(&word(interrupted(f.state)));
    }
    rows.push_str(&match (run, failure) {
        (_, Some((run, failure))) => failed(run, failure),
        (Some(run), None) => going(run),
        (None, _) if m.asking.as_deref() == Some(f.name.as_str()) => asking_remove(f),
        // Nothing to offer one who may not: why is said, once.
        (None, _) if !f.may_change => String::new(),
        (None, _) => {
            let enabled = !m.busy();
            bottom(&settings::actions(&changes(f).into_iter().map(|(label, verb, kind)| change(label, verb, f, kind, enabled)).collect::<String>()))
        }
    });
    let foot = if !f.may_change && !banner { settings::locked(DENIED) } else { String::new() };
    settings::group("", &rows, &foot)
}

/// feat's words as a sentence: capitalised, and ending in a full stop.
pub fn sentence(text: &str) -> String {
    let text = text.trim();
    let mut chars = text.chars();
    let mut out = match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => return String::new(),
    };
    if !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(state: State) -> Feature {
        Feature { name: "x".into(), state, defined: true, may_change: true, ..Default::default() }
    }

    #[test]
    fn each_state_offers_its_way_on_last() {
        let last = |state| changes(&feature(state)).last().map(|(label, verb, _)| (*label, *verb));
        assert_eq!(last(State::NotInstalled), Some(("Turn On", Verb::Add)));
        assert_eq!(last(State::Enabled), Some(("Turn Off", Verb::Disable)));
        assert_eq!(last(State::Installing), Some(("Try Again", Verb::Install)));
        assert_eq!(last(State::Uninstalling), Some(("Try Again", Verb::Remove)));
    }

    #[test]
    fn a_script_failure_offers_try_again_and_a_refusal_does_not() {
        let run = Run {
            number: 1,
            name: "x".into(),
            shown: "X".into(),
            verb: Verb::Enable,
            progress: Some(("enable".into(), 1, 1)),
            output: vec![],
            failed: None,
        };
        let script = failure(&run, &Failure { code: "script".into(), message: String::new() });
        assert_eq!(script.0, "Its enable script failed");
        assert!(script.2);
        assert!(!failure(&run, &Failure { code: "denied".into(), message: String::new() }).2);
        let state = failure(&run, &Failure { code: "state".into(), message: "feature x is not installed (run `feat install x`)".into() });
        assert_eq!(state.1, "Feature x is not installed.");
    }

    #[test]
    fn words_become_sentences() {
        assert_eq!(sentence("no such feature: x"), "No such feature: x.");
        assert_eq!(sentence("Done."), "Done.");
        assert_eq!(sentence(""), "");
    }
}
