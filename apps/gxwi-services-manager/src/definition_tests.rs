use peinit::client::{RawRegistryValue, RegistryValueType};

use super::*;

fn editor(values: &[(&str, &str)], may_change: Result<(), String>) -> Editor {
    let mut definition = Definition::new("web", Vec::new());
    for (field, text) in values {
        definition.set(field, text).unwrap();
    }
    Editor::with(true, definition, may_change, Ok(()), None)
}

fn shown(editor: &Editor) -> String {
    editor.render(&Facts {
        views: 1,
        fields: &Fields::default(),
    })
}

fn typed(editor: &mut Editor, field: &str, text: &str) {
    let mut fields = Fields::default();
    let name = format!("f-{field}");
    fields.set(&name, text);
    editor.input(&name, &mut fields);
}

#[test]
fn each_timer_says_when_its_schedule_comes_round_and_one_that_never_does_says_so() {
    let mut editor = editor(&[("ImagePath", "/usr/bin/web")], Ok(()));
    // Saturday 3 October 2026, 12:20 UTC, on the machine's own clock: a
    // schedule with no zone is in the machine's zone, which a build root
    // (often with no zone database) and a desk need not agree on, so the
    // clock is read in the same one.
    editor.clock = words::Clock::Fixed(
        "2026-10-03T12:20:00Z".parse().unwrap(),
        jiff::tz::TimeZone::system(),
    );
    assert!(!shown(&editor).contains("class=\"schedules\""));
    typed(
        &mut editor,
        "Triggers",
        "boot\ntimer:*-*-* 02:00:00\ntimer:*-02-30",
    );
    let html = shown(&editor);
    let said = html
        .split("<ul class=\"schedules\">")
        .nth(1)
        .unwrap_or(&html);
    assert!(
        said.starts_with("<li><code>*-*-* 02:00:00</code> comes round next ")
            && said.contains(" at 02:00.</li>"),
        "{said}"
    );
    assert!(said.contains("<li class=\"bad\"><code>*-02-30</code> never comes round, so it never starts the service.</li></ul>"), "{said}");
    // It is said, not refused: peinit takes it, and arms the rest.
    assert!(html.contains("fx-click=\"save\" fx-key=\"Ctrl+S\">Save"));
}

#[test]
fn every_field_is_shown_by_group_with_what_it_does_and_its_default() {
    let editor = editor(
        &[("ImagePath", "/usr/bin/web"), ("DisplayName", "Web")],
        Ok(()),
    );
    let html = shown(&editor);
    assert_eq!(html.matches("<div class=\"field").count(), FIELDS.len());
    assert!(html.contains("<h1>Web definition</h1>") && html.contains("<p class=\"id\">web</p>"));
    assert!(html.contains("<label for=\"f-ImagePath\">Program <code>ImagePath</code></label>"));
    assert!(html.contains(
        "The program to run, as a path from the root. A change takes effect when it is restarted."
    ));
    assert!(
        html.contains("placeholder=\"30\""),
        "a number's default is its placeholder"
    );
    assert!(html.contains("<option value=\"\">Default (Simple)</option><option value=\"Simple\">Simple</option><option value=\"Oneshot\">Oneshot</option>"));
    assert!(html.contains("<h2 id=\"g-Dependencies\">Dependencies</h2>"));
    assert!(
        html.contains("No changes.")
            && html.contains("fx-click=\"save\" fx-key=\"Ctrl+S\" disabled>Save")
    );
}

#[test]
fn what_is_wrong_is_said_beside_its_field_and_it_cannot_be_saved() {
    let mut editor = editor(&[("ImagePath", "/usr/bin/web")], Ok(()));
    typed(&mut editor, "StopTimeout", "ten");
    let html = shown(&editor);
    assert!(html.contains("<p class=\"wrong\" id=\"w-StopTimeout\">StopTimeout is a whole number, from 0 to 4294967295.</p>"));
    assert!(
        html.contains("aria-describedby=\"h-StopTimeout w-StopTimeout\" aria-invalid=\"true\"")
    );
    assert!(html.contains("Stop timeout is not right yet.") && html.contains(" disabled>Save"));
    typed(&mut editor, "StopTimeout", "20");
    typed(&mut editor, "ImagePath", "web");
    let html = shown(&editor);
    assert!(
        html.contains("<p class=\"wrong\" id=\"w-ImagePath\">ImagePath is a path from the root")
    );
    assert!(html.contains("The service manager would not take this: "));
    typed(&mut editor, "ImagePath", "/usr/bin/web");
    let html = shown(&editor);
    assert!(
        html.contains("The service manager would take this.")
            && html.contains("fx-key=\"Ctrl+S\">Save")
    );
    assert!(html.contains("<div class=\"field changed\"><label for=\"f-StopTimeout\">"));
    // Closing now asks first.
    assert!(!editor.closing(&mut Fields::default()));
    assert!(shown(&editor).contains("Close without saving what has changed?"));
}

#[test]
fn a_definition_that_may_not_be_changed_is_shown_fixed_and_says_why() {
    let mut editor = editor(
        &[("ImagePath", "/usr/bin/web")],
        Err("you are not allowed to change its definition".into()),
    );
    editor.edited.values.push(RawRegistryValue {
        name: "LastTimerRun".into(),
        value_type: RegistryValueType::Other(11),
        data: vec![0; 8],
    });
    let html = shown(&editor);
    assert!(html.contains(
        "You may read this definition, not change it: you are not allowed to change its definition."
    ));
    assert!(html.contains("<input id=\"f-ImagePath\" name=\"f-ImagePath\" autocomplete=\"off\" spellcheck=\"false\" placeholder=\"\" aria-describedby=\"h-ImagePath\" disabled>"));
    assert!(!html.contains(">Save<") && !html.contains(">Revert<"));
    assert!(html.contains(
        "<code>LastTimerRun</code> when its timer last ran, kept by the service manager"
    ));
    typed(&mut editor, "ImagePath", "/elsewhere");
    assert_eq!(
        editor.edited.text("ImagePath").as_deref(),
        Some("/usr/bin/web"),
        "nothing typed counts"
    );
    assert!(editor.closing(&mut Fields::default()));
}

#[test]
fn a_new_service_needs_a_name_and_a_program() {
    let mut editor = Editor::with(
        false,
        Definition::new("", Vec::new()),
        Ok(()),
        Err("it is not defined yet".into()),
        None,
    );
    let html = shown(&editor);
    assert!(
        html.contains("<h1>New service</h1>") && html.contains("<input id=\"name\" name=\"name\"")
    );
    assert!(html.contains(">Define</button>") && !html.contains("Delete…"));
    let mut fields = Fields::default();
    fields.set("name", "my web");
    editor.input("name", &mut fields);
    assert!(
        shown(&editor).contains("<p class=\"wrong\" id=\"w-name\">“my web” is not a service name")
    );
    fields.set("name", "web");
    editor.input("name", &mut fields);
    assert!(shown(&editor).contains(
        "<p class=\"wrong\" id=\"w-ImagePath\">It needs a program to run: ImagePath is not set.</p>"
    ));
    typed(&mut editor, "ImagePath", "/usr/bin/web");
    assert!(shown(&editor).contains("fx-key=\"Ctrl+S\">Define</button>"));
    assert_eq!(
        list(&["Runs as", "Privileges", "Program"]),
        "Runs as, Privileges and Program"
    );
}
