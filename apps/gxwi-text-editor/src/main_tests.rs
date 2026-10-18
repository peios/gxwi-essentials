use super::*;

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gxwi-text-editor-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An editor on `path` with its field filled, as `main` makes one.
fn editor(path: Option<&Path>) -> (Editor, Fields) {
    let (editor, text) = Editor::on(path.map(Path::to_path_buf));
    let mut fields = Fields::default();
    fields.set("text", &text);
    (editor, fields)
}

fn shown(editor: &Editor, fields: &Fields) -> String {
    editor.render(&Facts { views: 1, fields })
}

fn state(editor: &Editor, fields: &Fields) -> String {
    let html = shown(editor, fields);
    html.split("data-state=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_string()
}

/// Makes the file other than it was, whatever the clock says.
fn change(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
}

#[test]
fn what_is_typed_is_saved_to_the_file_and_not_before() {
    let dir = dir("saving");
    let path = dir.join("notes.txt");
    std::fs::write(&path, "first\n").unwrap();
    let (mut editor, mut fields) = editor(Some(&path));
    assert_eq!(fields.get("text"), "first\n");
    assert_eq!(state(&editor, &fields), "Saved");
    assert!(editor.closing(&mut fields));

    fields.set("text", "first\nsecond\n");
    assert_eq!(state(&editor, &fields), "Not saved");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\n");
    assert!(shown(&editor, &fields).contains("2 lines · 13 B · LF"));
    editor.event("save", &Value::Null, &mut fields);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\nsecond\n");
    assert_eq!(state(&editor, &fields), "Saved");
    // Typed and then taken back, there is nothing to save.
    fields.set("text", "other");
    fields.set("text", "first\nsecond\n");
    assert_eq!(state(&editor, &fields), "Saved");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_new_file_is_saved_where_the_person_says_and_over_nothing() {
    let dir = dir("new");
    std::fs::write(dir.join("taken.txt"), "kept").unwrap();
    let (mut editor, mut fields) = editor(None);
    assert_eq!(
        (editor.name().as_str(), state(&editor, &fields).as_str()),
        ("Untitled", "New")
    );
    fields.set("text", "words");
    editor.event("save", &Value::Null, &mut fields);
    assert_eq!(editor.asking, Some(Asking::Path { closing: false }));
    assert!(fields.get("path").ends_with("/untitled.txt"));
    assert!(shown(&editor, &fields).contains("<input id=\"path\" name=\"path\" fx-autofocus"));

    fields.set("path", &dir.join("taken.txt").to_string_lossy());
    editor.event("saved-as", &Value::Null, &mut fields);
    assert!(
        editor
            .trouble
            .as_deref()
            .unwrap()
            .starts_with("There is already something at")
    );
    assert_eq!(editor.asking, Some(Asking::Path { closing: false }));
    assert_eq!(
        std::fs::read_to_string(dir.join("taken.txt")).unwrap(),
        "kept"
    );
    for not_a_name in ["", "  ", "somewhere/"] {
        fields.set("path", not_a_name);
        editor.event("saved-as", &Value::Null, &mut fields);
        assert_eq!(
            editor.trouble.as_deref(),
            Some("That is not a name a file can have."),
            "{not_a_name:?}"
        );
    }
    fields.set("path", &dir.join("nowhere/made.txt").to_string_lossy());
    editor.event("saved-as", &Value::Null, &mut fields);
    assert!(
        editor
            .trouble
            .as_deref()
            .unwrap()
            .ends_with("there is no such directory.")
    );

    fields.set("path", &dir.join("made.txt").to_string_lossy());
    editor.event("saved-as", &Value::Null, &mut fields);
    assert_eq!((editor.trouble.as_deref(), editor.asking), (None, None));
    assert_eq!(
        std::fs::read_to_string(dir.join("made.txt")).unwrap(),
        "words"
    );
    assert_eq!(
        (editor.name().as_str(), state(&editor, &fields).as_str()),
        ("made.txt", "Saved")
    );
    // Another name beside it, said without where.
    editor.event("save-as", &Value::Null, &mut fields);
    assert_eq!(fields.get("path"), dir.join("made.txt").to_string_lossy());
    fields.set("path", "beside.txt");
    editor.event("saved-as", &Value::Null, &mut fields);
    assert_eq!(
        std::fs::read_to_string(dir.join("beside.txt")).unwrap(),
        "words"
    );
    assert_eq!(
        editor.path.as_deref(),
        Some(dir.join("beside.txt").as_path())
    );
    // Asked and then not: nothing is saved.
    editor.event("save-as", &Value::Null, &mut fields);
    editor.event("cancel", &Value::Null, &mut fields);
    fields.set("path", &dir.join("never.txt").to_string_lossy());
    editor.event("saved-as", &Value::Null, &mut fields);
    assert!(!dir.join("never.txt").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_file_named_and_not_there_is_made_by_saving() {
    let dir = dir("named");
    let path = dir.join("later.txt");
    let (mut editor, mut fields) = editor(Some(&path));
    assert_eq!(
        (editor.name().as_str(), state(&editor, &fields).as_str()),
        ("later.txt", "New")
    );
    assert!(!path.exists());
    fields.set("text", "now");
    editor.event("save", &Value::Null, &mut fields);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "now");
    assert_eq!(state(&editor, &fields), "Saved");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn closing_asks_when_there_is_something_to_lose() {
    let dir = dir("closing");
    let path = dir.join("notes.txt");
    std::fs::write(&path, "first").unwrap();
    let (mut editor, mut fields) = editor(Some(&path));
    fields.set("text", "changed");
    assert!(!editor.closing(&mut fields));
    let html = shown(&editor, &fields);
    assert!(html.contains("<p>notes.txt has changes that are not saved.</p>"));
    assert!(html.contains("fx-click=\"close-saved\" fx-autofocus"));
    editor.event("cancel", &Value::Null, &mut fields);
    assert_eq!(editor.asking, None);
    // Not asked, nothing is discarded or saved by a page saying so.
    editor.event("close-saved", &Value::Null, &mut fields);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
    assert!(!editor.closing(&mut fields));
    editor.event("close-saved", &Value::Null, &mut fields);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "changed");
    assert!(editor.closing(&mut fields));

    // One with no name is asked where, and goes once it is saved.
    let (mut editor, mut fields) = self::editor(None);
    fields.set("text", "words");
    assert!(!editor.closing(&mut fields));
    editor.event("close-saved", &Value::Null, &mut fields);
    assert_eq!(editor.asking, Some(Asking::Path { closing: true }));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn what_something_else_does_to_the_file_is_taken_in_or_asked_about() {
    let dir = dir("changed");
    let path = dir.join("notes.txt");
    std::fs::write(&path, "first").unwrap();
    let (mut editor, mut fields) = editor(Some(&path));
    assert!(!editor.differs());
    // Nothing changed here: what is there now is what is shown.
    change(&path, "theirs, which is longer");
    assert!(editor.differs());
    editor.look(&mut fields);
    assert_eq!(
        (fields.get("text"), editor.asking),
        ("theirs, which is longer", None)
    );
    assert_eq!(state(&editor, &fields), "Saved");

    // Changed here as well: the person says which.
    fields.set("text", "mine");
    change(&path, "theirs again, and longer still");
    editor.look(&mut fields);
    assert_eq!(
        (fields.get("text"), editor.asking),
        ("mine", Some(Asking::Changed))
    );
    assert!(!editor.differs());
    editor.event("keep", &Value::Null, &mut fields);
    editor.event("save", &Value::Null, &mut fields);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine");

    fields.set("text", "mine once more");
    change(&path, "theirs a third time, longer");
    // Saving notices too, and writes nothing until the person has said.
    editor.event("save", &Value::Null, &mut fields);
    assert_eq!(editor.asking, Some(Asking::Changed));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "theirs a third time, longer"
    );
    editor.event("load", &Value::Null, &mut fields);
    assert_eq!(fields.get("text"), "theirs a third time, longer");

    // Gone, it is said to be, and saving puts it back.
    std::fs::remove_file(&path).unwrap();
    editor.look(&mut fields);
    assert_eq!(
        editor.trouble.as_deref(),
        Some("notes.txt is no longer there. Saving puts it back.")
    );
    assert_eq!(state(&editor, &fields), "Not saved");
    editor.event("save", &Value::Null, &mut fields);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "theirs a third time, longer"
    );
    assert_eq!(state(&editor, &fields), "Saved");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn what_is_not_text_is_not_shown_and_cannot_be_saved() {
    let dir = dir("refused");
    let path = dir.join("image.bin");
    std::fs::write(&path, b"\x89PNG\0\0").unwrap();
    let (mut editor, mut fields) = editor(Some(&path));
    let html = shown(&editor, &fields);
    assert!(html.contains("This file is not text"));
    assert!(!html.contains("<textarea"));
    assert_eq!(state(&editor, &fields), "Not shown");
    fields.set("text", "whatever a page says");
    editor.event("save", &Value::Null, &mut fields);
    editor.event("save-as", &Value::Null, &mut fields);
    assert_eq!(editor.asking, None);
    assert_eq!(std::fs::read(&path).unwrap(), b"\x89PNG\0\0");
    assert!(editor.closing(&mut fields));
    std::fs::remove_dir_all(dir).unwrap();
}
