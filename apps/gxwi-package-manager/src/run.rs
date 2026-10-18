//! A change being made: peipkg's driven run, from the plan through each
//! question it asks to how it ended, and the page that shows it.
//!
//! Everything peipkg asks is put to the person as it asks it, on its own:
//! each elevated action has its own Allow, the plan its own Install or
//! Remove, and a changed configuration file its own keep-or-delete, with
//! the safe answer the keyboard's. Nothing is answered for them, except
//! the proceed of a change they have already chosen in a list.

use libgxwi::escape;
use libgxwi::settings::{self, Glyph, Kind, More, Tile, Tone};

use crate::peipkg::{Answers, Event, Operation, Plan, Progress, Question};
use crate::words;

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    Done(String),
    Cancelled(String),
    /// peipkg's code for why, and its words.
    Failed { code: String, message: String },
}

pub struct Run {
    /// Which run this is, so a run's late events can't land on the next.
    pub number: u64,
    /// What it is called: "Install make".
    pub title: String,
    /// The proceed button's word: "Install".
    pub verb: &'static str,
    /// What peipkg was run with, and what a retry added.
    pub args: Vec<String>,
    /// Proceed is answered as soon as it is asked: the person chose this
    /// change already, in a list.
    pub chosen: bool,
    pub plan: Option<Plan>,
    pub question: Option<Question>,
    pub progress: Option<Progress>,
    /// What peipkg said last that wasn't a warning, while it works.
    pub doing: Option<String>,
    pub warnings: Vec<String>,
    pub end: Option<Ended>,
    pub answers: Option<Answers>,
    pub proceeded: bool,
    pub notes_open: bool,
    /// What it did, said when it is done, for a change with no plan to say
    /// it from: "mirror removed."
    pub said: Option<String>,
}

impl Run {
    pub fn new(number: u64, title: String, verb: &'static str, args: Vec<String>, chosen: bool) -> Run {
        Run {
            number,
            title,
            verb,
            args,
            chosen,
            plan: None,
            question: None,
            progress: None,
            doing: None,
            warnings: Vec::new(),
            end: None,
            answers: None,
            proceeded: false,
            notes_open: false,
            said: None,
        }
    }

    /// Whether peipkg is still at it.
    pub fn going(&self) -> bool {
        self.end.is_none()
    }

    /// Whether it is past the point of being called off: the plan is
    /// approved and the changes are being made.
    pub fn committed(&self) -> bool {
        self.proceeded && self.going()
    }

    /// Takes in one event.
    pub fn heard(&mut self, event: Event) {
        match event {
            Event::Plan(plan) => self.plan = Some(plan),
            Event::Question(question) => {
                if question.kind == "proceed" && self.chosen {
                    self.question = Some(question.clone());
                    self.answer(question.id, "yes");
                    return;
                }
                self.question = Some(question);
            }
            Event::Progress(progress) => {
                self.question = None;
                self.progress = Some(progress);
            }
            Event::Warning { text } => self.warnings.push(text),
            Event::Message { text } => self.doing = Some(text),
            Event::Done { summary, .. } => {
                self.question = None;
                self.end = Some(Ended::Done(self.done_words(summary.as_deref())));
            }
            Event::Cancelled { reason } => {
                self.question = None;
                self.end = Some(Ended::Cancelled(reason));
            }
            Event::Error { code, message } => {
                self.question = None;
                self.end = Some(Ended::Failed { code, message });
            }
        }
    }

    /// Answers the open question.
    pub fn answer(&mut self, id: u64, answer: &str) {
        if let Some(answers) = &self.answers {
            answers.answer(id, answer);
        }
        if self.question.as_ref().is_some_and(|q| q.kind == "proceed" && q.id == id) && answer == "yes" {
            self.proceeded = true;
        }
        self.question = None;
    }

    /// What was done, in words, from the plan where there is one.
    fn done_words(&self, summary: Option<&str>) -> String {
        if self.plan.is_none()
            && let Some(said) = &self.said
        {
            return said.clone();
        }
        let ops = self.plan.as_ref().map(|p| p.operations.as_slice()).unwrap_or_default();
        match (ops, summary) {
            ([], Some("nothing to do")) => "Nothing needed doing: it's already as asked.".into(),
            ([], Some(summary)) => words::sentence(summary),
            ([], None) => "Done.".into(),
            ([op], _) => {
                let to = op.to.as_deref().unwrap_or_default();
                let name = words::short(&op.name);
                match op.kind.as_str() {
                    "install" => format!("{name} {to} installed."),
                    "remove" => format!("{name} removed."),
                    "upgrade" => format!("{name} updated to {to}."),
                    _ => format!("{name} moved back to {to}."),
                }
            }
            (ops, _) => format!("{} made.", words::count(ops.len(), "change", "changes")),
        }
    }

    /// Where it has got to, as a bar: fetching and staging step through the
    /// packages coming in, then three steps for the whole.
    fn fraction(&self) -> (usize, usize) {
        let coming = self.plan.as_ref().map_or(0, |p| p.operations.iter().filter(|o| o.kind != "remove").count());
        let total = coming * 2 + 3;
        let Some(p) = &self.progress else { return (0, total) };
        let done = match p.phase.as_str() {
            "fetch" => p.step.saturating_sub(1),
            "stage" => coming + p.step.saturating_sub(1),
            "apply" => coming * 2,
            "commit" => coming * 2 + 1,
            _ => coming * 2 + 2,
        };
        (done, total)
    }
}

/// What a step of the transaction is doing, in words.
fn doing(p: &Progress) -> String {
    let name = p.package.as_deref().map(words::short).unwrap_or_default();
    let of = if p.steps > 1 { format!(" ({} of {})", p.step, p.steps) } else { String::new() };
    match p.phase.as_str() {
        "fetch" => format!("Downloading and checking {name}{of}…"),
        "stage" => format!("Preparing {name}{of}…"),
        "apply" => "Putting the changes in place…".into(),
        "commit" => "Recording the changes…".into(),
        _ => "Finishing up…".into(),
    }
}

/// One change of the plan, as a row.
fn operation(op: &Operation) -> String {
    let (glyph, tile) = match op.kind.as_str() {
        "install" => (Glyph::Download, Tile::Blue),
        "remove" => (Glyph::Blocked, Tile::Red),
        "upgrade" => (Glyph::Refresh, Tile::Green),
        _ => (Glyph::Refresh, Tile::Orange),
    };
    let from = op.from.as_deref().unwrap_or_default();
    let to = op.to.as_deref().unwrap_or_default();
    let what = match op.kind.as_str() {
        "install" => format!("Install {to}"),
        "remove" => format!("Remove {from}"),
        "upgrade" => format!("Update {from} → {to}"),
        _ => format!("Move back {from} → {to}"),
    };
    let mut lines = vec![op.name.clone()];
    if op.local {
        lines.push("From a file, not a repository".into());
    }
    if let Some(root) = &op.root {
        lines.push(format!("In {root}"));
    }
    let lines: Vec<(&str, bool)> = lines.iter().enumerate().map(|(i, l)| (l.as_str(), i == 0)).collect();
    let size = if op.kind == "remove" { String::new() } else { words::size(op.size_installed) };
    let control = format!(r#"<span class="what">{}</span><span class="size">{}</span>"#, escape(&what), escape(&size));
    settings::item(&settings::icon(glyph, tile), words::short(&op.name), &lines, &control)
}

/// The plan, as rows, with what it costs under them.
fn plan(plan: &Plan) -> String {
    let rows: String = plan.operations.iter().map(operation).collect();
    let download: i64 = plan.operations.iter().filter(|o| o.kind != "remove" && !o.local).map(|o| o.size_download).sum();
    let coming: i64 = plan.operations.iter().filter(|o| o.kind != "remove").map(|o| o.size_installed).sum();
    let foot = if download > 0 {
        settings::hint(&format!("Downloads {}, and takes {} once installed.", words::size(download), words::size(coming)))
    } else {
        String::new()
    };
    let mut out = settings::group(&words::count(plan.operations.len(), "Change", "Changes"), &rows, &foot);
    if !plan.other_roots.is_empty() {
        out.push_str(&settings::caution(&format!(
            "This also changes software in other installation roots: {}.",
            plan.other_roots.join(", ")
        )));
    }
    for note in &plan.notes {
        out.push_str(&settings::caution(&words::sentence(note)));
    }
    out
}

/// A button the keyboard goes to as it appears, sending `values`.
pub fn focused(label: &str, event: &str, values: &[(&str, &str)], kind: Kind) -> String {
    settings::button(label, event, values, kind, true).replacen("<button ", "<button fx-autofocus ", 1)
}

/// The open question, with its answers.
fn question(run: &Run, q: &Question) -> String {
    let id = q.id.to_string();
    let values = [("id", id.as_str())];
    match q.kind.as_str() {
        "authorise" => settings::group(
            "Needs Your Approval",
            &settings::more(
                More::Asking,
                &format!(
                    "<p>{}</p><p class=\"quiet\">This is allowed only when you say so, and your approval is recorded.</p>{}",
                    escape(&words::sentence(&q.text)),
                    settings::actions(&format!(
                        "{}{}",
                        focused("Cancel", "refuse", &values, Kind::Plain),
                        settings::button("Allow This", "allow", &values, Kind::Primary, true)
                    ))
                ),
            ),
            "",
        ),
        "modified" => {
            let name = words::short(&q.package);
            settings::group(
                "A Changed File",
                &settings::more(
                    More::Asking,
                    &format!(
                        "<p><code class=\"st-mono\">{}</code> has been changed since {} was installed. Removing {} would delete it.</p><p class=\"quiet\">Keep it, and it stays as it is, belonging to no package. Delete it, and a copy is kept beside it.</p>{}",
                        escape(&q.path),
                        escape(name),
                        escape(name),
                        settings::actions(&format!(
                            "{}{}{}",
                            settings::button("Stop Everything", "modified", &[("id", &id), ("answer", "abort")], Kind::Quiet, true),
                            settings::button("Delete It", "modified", &[("id", &id), ("answer", "remove")], Kind::Danger, true),
                            focused("Keep It", "modified", &[("id", &id), ("answer", "keep")], Kind::Primary),
                        ))
                    ),
                ),
                "",
            )
        }
        _ => {
            let removing = run.plan.as_ref().is_some_and(|p| p.operations.iter().any(|o| o.kind == "remove"));
            // Taking software away is the one the keyboard doesn't start on.
            let go = if removing {
                settings::button(run.verb, "proceed", &values, Kind::Danger, true)
            } else {
                focused(run.verb, "proceed", &values, Kind::Primary)
            };
            let stay = if removing { focused("Cancel", "refuse", &values, Kind::Plain) } else { settings::button("Cancel", "refuse", &values, Kind::Plain, true) };
            settings::actions(&format!("{stay}{go}"))
        }
    }
}

/// What a failure means, and the way on where there is one: a retry with
/// the flag that allows it.
fn failure(code: &str, message: &str, may_retry: bool) -> (String, String, Option<(&'static str, &'static str)>) {
    let (title, about, retry) = match code {
        "stale" => (
            "A repository's information is out of date",
            "Refreshing it didn't bring anything newer. You can carry on with what this machine already has, which may lack recent fixes. Carrying on is recorded.",
            Some(("Continue Anyway", "--allow-stale")),
        ),
        "busy" => ("Another change is being made", "Software is already being installed or removed. Try again once that has finished.", Some(("Try Again", ""))),
        "denied" => ("You can't make this change", "Changing software needs write access to where it is installed. As shipped, only Administrators have it.", None),
        "unowned" => (
            "A file is in the way",
            "A file this would install is already there, and no package owns it. Replacing it keeps the file that's there beside the new one.",
            Some(("Replace It", "--overwrite-unowned")),
        ),
        "alternate-upgrade" => (
            "This is updated another way",
            "Its publisher says to update it with something other than Package Manager. Updating it here may skip steps they rely on.",
            Some(("Update Anyway", "--bypass-alternate-upgrade")),
        ),
        "unresolvable" => ("This can't be done", "What it needs can't all be had together.", None),
        // Not a retry: the way on is elsewhere, and is a choice of its own.
        "untrusted" => (
            "A repository isn't trusted yet",
            "It's set up on this machine, but its trust hasn't been confirmed, so nothing can be installed or updated until it is. Confirm it, or remove it, under Repositories.",
            Some(("Repositories", "section:repositories")),
        ),
        _ => ("The change failed", "", None),
    };
    let retry = if may_retry { retry } else { None };
    let mut about = about.to_string();
    let detail = words::detail(message);
    if !detail.is_empty() {
        if !about.is_empty() {
            about.push(' ');
        }
        about.push_str(&words::sentence(detail));
    }
    (title.to_string(), about, retry)
}

/// The run, as the page.
pub fn page(run: &Run) -> String {
    let mut page = settings::head(Glyph::Package, Tile::Orange, &run.title, "");
    let failed = matches!(run.end, Some(Ended::Failed { .. }));
    if let Some(p) = &run.plan
        && !p.operations.is_empty()
        && !failed
    {
        page.push_str(&plan(p));
    }
    match (&run.end, &run.question) {
        (None, Some(q)) => page.push_str(&question(run, q)),
        (None, None) => {
            let (done, of) = run.fraction();
            let text = match (&run.progress, &run.doing) {
                (Some(p), _) => doing(p),
                (None, Some(said)) => words::sentence(said),
                (None, None) => "Working out what's needed…".into(),
            };
            let bar = if run.progress.is_some() { settings::progress(done, of, &text) } else { settings::progress(0, 0, &text) };
            page.push_str(&settings::group("", &bar, ""));
        }
        (Some(Ended::Done(said)), _) => {
            page.push_str(&settings::hero(&settings::big("Done", said), &settings::pill("Finished", Tone::Good)));
            page.push_str(&settings::actions(&settings::focused_button("Close", "dismiss", Kind::Primary)));
        }
        (Some(Ended::Cancelled(_)), _) => {
            page.push_str(&settings::hero(&settings::big("Cancelled", "Nothing was changed."), ""));
            page.push_str(&settings::actions(&settings::focused_button("Close", "dismiss", Kind::Primary)));
        }
        (Some(Ended::Failed { code, message }), _) => {
            let flagged = run.args.iter().any(|a| a.starts_with("--allow-") || a.starts_with("--overwrite-") || a.starts_with("--bypass-"));
            // A flag is offered once: what it allows was refused anyway.
            let (title, about, retry) = failure(code, message, !flagged || code == "busy" || code == "untrusted");
            page.push_str(&settings::group("", &settings::row(&title, &about, ""), ""));
            let retry = match retry {
                Some((label, flag)) => match flag.strip_prefix("section:") {
                    Some(section) => settings::button(label, "section", &[("section", section)], Kind::Primary, true),
                    None => settings::button(label, "retry", &[("flag", flag)], Kind::Primary, true),
                },
                None => String::new(),
            };
            page.push_str(&settings::actions(&format!("{}{retry}", settings::focused_button("Close", "dismiss", Kind::Plain))));
        }
    }
    page.push_str(&notes(run));
    page
}

/// What peipkg warned of, each once: an update of everything warns of
/// every orphaned package it passes over, which is said in one line.
fn gathered(warnings: &[String]) -> Vec<String> {
    const ORPHANED: &str = " is orphaned — ";
    let mut out: Vec<String> = Vec::new();
    for w in warnings.iter().filter(|w| !w.contains(ORPHANED)) {
        let w = words::sentence(w);
        if !out.contains(&w) {
            out.push(w);
        }
    }
    let orphans = warnings.iter().filter(|w| w.contains(ORPHANED)).count();
    if orphans > 0 {
        out.push(format!(
            "{} came from a repository this machine no longer uses, and none offers {} now, so {} left as {}.",
            words::count(orphans, "package", "packages"),
            if orphans == 1 { "it" } else { "them" },
            if orphans == 1 { "it was" } else { "they were" },
            if orphans == 1 { "it is" } else { "they are" },
        ));
    }
    out
}

/// What peipkg warned of, folded away when there is a lot of it.
fn notes(run: &Run) -> String {
    let notes = gathered(&run.warnings);
    if notes.is_empty() {
        return String::new();
    }
    const SHOWN: usize = 3;
    let open = run.notes_open || notes.len() <= SHOWN;
    let shown = if open { notes.len() } else { SHOWN };
    let rows: String = notes[..shown].iter().map(|w| format!(r#"<div class="st-row note"><span class="label"><small>{}</small></span></div>"#, escape(w))).collect();
    let foot = if open { String::new() } else { settings::actions(&settings::button(&format!("Show All {}", notes.len()), "notes", &[], Kind::Quiet, true)) };
    settings::group(&words::count(notes.len(), "Note", "Notes"), &rows, &foot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(kind: &str, name: &str, from: Option<&str>, to: Option<&str>) -> Operation {
        Operation { kind: kind.into(), name: name.into(), from: from.map(Into::into), to: to.map(Into::into), ..Default::default() }
    }

    #[test]
    fn what_was_done_is_said_from_the_plan() {
        let mut run = Run::new(1, "Install make".into(), "Install", vec![], false);
        run.heard(Event::Plan(Plan { operations: vec![op("install", "org.gnu.make", None, Some("4.4.1-1"))], ..Default::default() }));
        run.heard(Event::Done { summary: Some("1 operation applied".into()), transaction: Some(3) });
        assert_eq!(run.end, Some(Ended::Done("make 4.4.1-1 installed.".into())));

        let mut run = Run::new(2, "Refresh".into(), "Refresh", vec![], false);
        run.heard(Event::Done { summary: None, transaction: None });
        assert_eq!(run.end, Some(Ended::Done("Done.".into())));
    }

    #[test]
    fn progress_moves_through_the_phases() {
        let mut run = Run::new(1, String::new(), "Install", vec![], false);
        run.heard(Event::Plan(Plan { operations: vec![op("install", "a", None, Some("1")), op("install", "b", None, Some("1"))], ..Default::default() }));
        run.heard(Event::Progress(Progress { phase: "fetch".into(), step: 2, steps: 2, package: Some("b".into()) }));
        assert_eq!(run.fraction(), (1, 7));
        run.heard(Event::Progress(Progress { phase: "commit".into(), step: 1, steps: 1, package: None }));
        assert_eq!(run.fraction(), (5, 7));
    }

    #[test]
    fn orphans_passed_over_are_one_note() {
        let warnings = vec![
            "a is orphaned — its repository is no longer configured".to_string(),
            "audit emission failed: x".to_string(),
            "b is orphaned — its repository is no longer configured".to_string(),
            "audit emission failed: x".to_string(),
        ];
        assert_eq!(
            gathered(&warnings),
            ["Audit emission failed: x.", "2 packages came from a repository this machine no longer uses, and none offers them now, so they were left as they are."]
        );
    }

    #[test]
    fn a_failure_offers_its_retry_once() {
        let (_, about, retry) = failure("stale", "repository \"x\" serves stale metadata\npass --allow-stale", true);
        assert_eq!(retry, Some(("Continue Anyway", "--allow-stale")));
        assert!(about.ends_with("Repository \"x\" serves stale metadata."));
        assert_eq!(failure("stale", "", false).2, None);
        assert_eq!(failure("unresolvable", "peipkg/resolver: package \"a\" depends on \"b\"", true).1, "What it needs can't all be had together. Package \"a\" depends on \"b\".");
    }
}
