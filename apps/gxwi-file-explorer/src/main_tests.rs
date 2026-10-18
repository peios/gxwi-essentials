use super::*;

/// A directory of its own with a few things in it, and a catalogue with
/// two applications that open text and one that opens directories.
fn explorer(name: &str) -> (Explorer, PathBuf) {
    let dir =
        std::env::temp_dir().join(format!("gxwi-file-explorer-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (files, apps) = (dir.join("files"), dir.join("apps"));
    std::fs::create_dir_all(files.join("Music/Albums")).unwrap();
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::write(files.join("notes.txt"), "notes").unwrap();
    std::fs::write(files.join("other.txt"), "other").unwrap();
    std::fs::write(files.join("Music/Albums/one.flac"), "").unwrap();
    // The program has to be a file that is there, and this one is.
    let program = std::env::current_exe().unwrap();
    for (id, title, opens) in [
        ("org.example.b", "Alpha", "text/*"),
        ("org.example.a", "Beta", "text/plain"),
        (
            "dev.peios.gxwi-file-explorer",
            "File Explorer",
            "inode/directory",
        ),
        ("org.example.none", "None", ""),
    ] {
        let opens = if opens.is_empty() {
            String::new()
        } else {
            format!("opens = [\"{opens}\"]\n")
        };
        std::fs::write(
            apps.join(format!("{id}.toml")),
            format!(
                "title = \"{title}\"\nprogram = \"{}\"\n{opens}",
                program.display()
            ),
        )
        .unwrap();
    }
    let held = dir.join("held");
    (
        Explorer::with(
            files,
            Catalogue::at(apps),
            Choosing::Fixed(Defaults::default()),
            Holding::at(Some(&held)),
        ),
        dir,
    )
}

/// Another window of the same session's, at `at`.
fn another(dir: &Path, at: &str) -> Explorer {
    Explorer::with(
        dir.join("files").join(at),
        Catalogue::at(dir.join("apps")),
        Choosing::Fixed(Defaults::default()),
        Holding::at(Some(&dir.join("held"))),
    )
}

/// Choices of what opens a type: the person's, and the machine's.
fn chosen(person: &[(&str, &str)], machine: &[(&str, &str)]) -> Choosing {
    let of = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(media, id)| (media.to_string(), id.to_string()))
            .collect()
    };
    Choosing::Fixed(Defaults {
        person: of(person),
        machine: of(machine),
    })
}

/// The labels of the menu `id` in what is shown.
fn menu_labels(html: &str, id: &str) -> Vec<String> {
    let menu = html
        .split(&format!("<menu id=\"{id}\" hidden>"))
        .nth(1)
        .unwrap()
        .split("</menu>")
        .next()
        .unwrap();
    menu.split("</button>")
        .filter_map(|item| item.rsplit('>').next())
        .filter(|label| !label.is_empty())
        .map(String::from)
        .collect()
}

fn shown(explorer: &Explorer) -> String {
    explorer.render(&Facts {
        views: 1,
        fields: &Fields::default(),
    })
}

fn names(explorer: &Explorer) -> Vec<&str> {
    explorer
        .entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect()
}

/// What comes with an event: the values an element was written with.
fn said(values: &[(&str, &str)]) -> Value {
    values
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

fn about(name: &str) -> Value {
    said(&[("name", name)])
}

#[test]
fn the_candidates_are_what_opens_the_type_by_title() {
    let (explorer, dir) = explorer("candidates");
    let titles = |name: &str| {
        explorer
            .candidates(&explorer.entries[explorer.named(name).unwrap()])
            .iter()
            .map(|a| a.title.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(titles("notes.txt"), ["Alpha", "Beta"]);
    assert_eq!(titles("Music"), ["File Explorer"]);
    let html = shown(&explorer);
    // One menu for the directory, one for the two files, and one for
    // where they all are.
    assert_eq!(html.matches("<menu id=").count(), 3);
    assert!(html.contains("fx-menu=\"menu-0\" fx-value-index=\"0\" fx-value-name=\"Music\""));
    assert!(html.contains("fx-menu=\"menu-1\" fx-value-index=\"1\" fx-value-name=\"notes.txt\""));
    assert!(html.contains("fx-menu=\"menu-1\" fx-value-index=\"2\" fx-value-name=\"other.txt\""));
    assert!(html.contains(&format!(
        "fx-value-path=\"{}\"",
        dir.join("files/notes.txt").display()
    )));
    let menu = html
        .split("<menu id=\"menu-1\" hidden>")
        .nth(1)
        .unwrap()
        .split("</menu>")
        .next()
        .unwrap();
    let labels: Vec<&str> = menu
        .split("</button>")
        .filter_map(|item| item.rsplit('>').next())
        .filter(|label| !label.is_empty())
        .collect();
    assert_eq!(
        labels,
        [
            "Open",
            "Open with Alpha",
            "Open with Beta",
            "Cut",
            "Copy",
            "Rename",
            "Copy path",
            "Permissions…",
            "Delete"
        ]
    );
    assert!(menu.contains("fx-copy=\"path\""));
    // The directory's own menu is the listing's, and opens it with
    // what opens directories.
    assert!(html.contains("<div class=\"listing\" id=\"listing\" fx-menu=\"menu-here\""));
    let menu = html
        .split("<menu id=\"menu-here\" hidden>")
        .nth(1)
        .unwrap()
        .split("</menu>")
        .next()
        .unwrap();
    let labels: Vec<&str> = menu
        .split("</button>")
        .filter_map(|item| item.rsplit('>').next())
        .filter(|label| !label.is_empty())
        .collect();
    assert_eq!(
        labels,
        ["New folder", "New file", "Paste", "Open with File Explorer"]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn what_opens_a_type_by_default_is_the_persons_choice_else_the_machines() {
    fn titles(explorer: &Explorer, name: &str) -> Vec<String> {
        explorer
            .candidates(&explorer.entries[explorer.named(name).unwrap()])
            .iter()
            .map(|a| a.title.clone())
            .collect()
    }
    let (mut explorer, dir) = explorer("defaults");
    // The machine's choice goes first, and opening uses it.
    explorer.choosing = chosen(&[], &[("text/plain", "org.example.a")]);
    explorer.reread();
    assert_eq!(titles(&explorer, "notes.txt"), ["Beta", "Alpha"]);
    assert_eq!(
        explorer
            .opening("text/plain")
            .first()
            .map(|a| a.id.as_str()),
        Some("org.example.a")
    );
    assert_eq!(
        menu_labels(&shown(&explorer), "menu-1"),
        [
            "Open",
            "Open with Beta",
            "Open with Alpha",
            "Cut",
            "Copy",
            "Rename",
            "Copy path",
            "Permissions…",
            "Delete"
        ]
    );
    // The person's goes before it, even for every type of text.
    explorer.choosing = chosen(
        &[("text/*", "org.example.b")],
        &[("text/plain", "org.example.a")],
    );
    explorer.reread();
    assert_eq!(titles(&explorer, "notes.txt"), ["Alpha", "Beta"]);
    // A choice of something that does not open the type is passed over,
    // and so is one of something not installed, and the rule is the
    // titles' again.
    explorer.choosing = chosen(
        &[("text/plain", "dev.peios.gxwi-file-explorer")],
        &[("text/*", "org.example.gone")],
    );
    explorer.reread();
    assert_eq!(titles(&explorer, "notes.txt"), ["Alpha", "Beta"]);
    assert_eq!(titles(&explorer, "Music"), ["File Explorer"]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_choices_are_read_again_when_a_file_is_opened() {
    let (mut explorer, dir) = explorer("chosen-now");
    let mut fields = Fields::default();
    assert!(explorer.defaults.person.is_empty());
    // Chosen elsewhere since the directory was read. Opening a file of
    // a type nothing opens starts nothing, and still reads them.
    explorer.choosing = chosen(&[("text/plain", "org.example.a")], &[]);
    explorer.event(
        "visit",
        &said(&[("path", &dir.join("files/Music/Albums").to_string_lossy())]),
        &mut fields,
    );
    explorer.choosing = chosen(&[("text/plain", "org.example.b")], &[]);
    explorer.event("open", &about("one.flac"), &mut fields);
    assert_eq!(
        explorer
            .defaults
            .person
            .get("text/plain")
            .map(String::as_str),
        Some("org.example.b")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_files_rights_each_have_all_of_the_ones_after_them_that_they_can() {
    let rights = file_rights(false);
    let mask = |name: &str| rights.iter().find(|right| right.name == name).unwrap().mask;
    let has = |more: &str, less: &str| mask(more) & mask(less) == mask(less);
    assert!(rights.iter().all(|right| right.mask != 0));
    assert_eq!(rights.iter().filter(|right| right.general).count(), 5);
    // Each right on its own is one bit, and Full control has all of them.
    let special: Vec<_> = rights.iter().filter(|right| !right.general).collect();
    assert!(
        special
            .iter()
            .all(|right| right.mask.count_ones() == 1 && has("Full control", &right.name))
    );
    assert!(
        file_rights(true)
            .iter()
            .any(|right| right.name == "Delete subfolders and files")
    );
    assert!(
        has("Full control", "Modify")
            && has("Modify", "Read & execute")
            && has("Read & execute", "Read")
            && has("Modify", "Write")
    );
    assert!(!has("Read", "Write") && !has("Modify", "Full control"));
    // As the kernel maps them, not as this guesses.
    assert_eq!(
        mask("Full control"),
        AccessMask::GENERIC_ALL
            .resolve_generic(&File::generic_mapping())
            .bits()
    );
}

#[test]
fn a_directory_is_opened_only_with_what_opens_directories() {
    let (mut explorer, dir) = explorer("here");
    let mut fields = Fields::default();
    explorer.event("open-here", &said(&[("app", "org.example.a")]), &mut fields);
    assert_eq!(
        explorer.trouble.as_deref(),
        Some("This directory cannot be opened with that.")
    );
    explorer.event(
        "open-here",
        &said(&[("app", "org.example.nowhere")]),
        &mut fields,
    );
    assert_eq!(
        explorer.trouble.as_deref(),
        Some("This directory cannot be opened with that.")
    );
    // What says nothing of its type and does not read as text is
    // opened by nothing.
    std::fs::write(dir.join("files/thing"), b"\x7fELF\0\0").unwrap();
    explorer.event("reread", &Value::Null, &mut fields);
    explorer.event("open", &about("thing"), &mut fields);
    assert_eq!(
        explorer.trouble.as_deref(),
        Some("Nothing installed says it opens files like this.")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_folder_or_a_file_is_made_by_the_name_typed_and_over_nothing() {
    let (mut explorer, dir) = explorer("making");
    let mut fields = Fields::default();
    explorer.event("new-folder", &Value::Null, &mut fields);
    assert_eq!(fields.get("name"), "New folder");
    let html = shown(&explorer);
    assert!(html.contains("<ul class=\"entries\"><li><form id=\"making\" class=\"directory renaming\" fx-submit=\"made\">"));
    assert!(!html.contains("fx-key=\"Enter\""));
    fields.set("name", "Pictures");
    explorer.event("made", &Value::Null, &mut fields);
    assert!(dir.join("files/Pictures").is_dir());
    assert_eq!(
        names(&explorer),
        ["Music", "Pictures", "notes.txt", "other.txt"]
    );
    assert_eq!(explorer.picked, explorer.named("Pictures"));

    explorer.event("new-file", &Value::Null, &mut fields);
    assert_eq!(fields.get("name"), "New file.txt");
    fields.set("name", "notes.txt");
    explorer.event("made", &Value::Null, &mut fields);
    assert_eq!(
        explorer.trouble.as_deref(),
        Some("There is already something called notes.txt here.")
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("files/notes.txt")).unwrap(),
        "notes"
    );
    explorer.event("new-file", &Value::Null, &mut fields);
    fields.set("name", "a/b");
    explorer.event("made", &Value::Null, &mut fields);
    assert_eq!(
        explorer.trouble.as_deref(),
        Some("That is not a name a file can have.")
    );
    explorer.event("new-file", &Value::Null, &mut fields);
    fields.set("name", "list.txt");
    explorer.event("made", &Value::Null, &mut fields);
    assert_eq!(
        std::fs::read_to_string(dir.join("files/list.txt")).unwrap(),
        ""
    );
    // Asked and then not: nothing is made.
    explorer.event("new-folder", &Value::Null, &mut fields);
    explorer.event("keep", &Value::Null, &mut fields);
    fields.set("name", "Never");
    explorer.event("made", &Value::Null, &mut fields);
    assert!(!dir.join("files/Never").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn what_is_copied_in_one_window_is_pasted_in_another_and_over_nothing() {
    let (mut explorer, dir) = explorer("copying");
    let mut fields = Fields::default();
    explorer.event("paste", &Value::Null, &mut fields);
    assert_eq!(
        explorer.trouble.as_deref(),
        Some("Nothing has been cut or copied.")
    );
    explorer.event("copy", &about("notes.txt"), &mut fields);
    assert!(!shown(&explorer).contains(" cut\""));
    // Where it came from, it is a copy by name.
    explorer.event("paste", &Value::Null, &mut fields);
    assert_eq!(
        names(&explorer),
        ["Music", "notes (copy).txt", "notes.txt", "other.txt"]
    );
    assert_eq!(explorer.picked, explorer.named("notes (copy).txt"));
    // Still held, for another window to paste.
    let mut other = another(&dir, "Music");
    other.event("paste", &Value::Null, &mut fields);
    other.event("paste", &Value::Null, &mut fields);
    assert_eq!(names(&other), ["Albums", "notes (copy).txt", "notes.txt"]);
    assert_eq!(
        std::fs::read_to_string(dir.join("files/Music/notes.txt")).unwrap(),
        "notes"
    );

    // A directory goes with everything in it, and not into itself.
    explorer.event("copy", &about("Music"), &mut fields);
    other.event("paste", &Value::Null, &mut fields);
    assert_eq!(
        other.trouble.as_deref(),
        Some("Music cannot be put inside itself.")
    );
    let mut deeper = another(&dir, "Music/Albums");
    deeper.event("paste", &Value::Null, &mut fields);
    assert_eq!(
        deeper.trouble.as_deref(),
        Some("Music cannot be put inside itself.")
    );
    explorer.event("paste", &Value::Null, &mut fields);
    assert_eq!(explorer.trouble, None);
    assert!(dir.join("files/Music (copy)/Albums/one.flac").exists());

    // What has gone since it was copied is said to have.
    explorer.event("copy", &about("other.txt"), &mut fields);
    std::fs::remove_file(dir.join("files/other.txt")).unwrap();
    other.event("paste", &Value::Null, &mut fields);
    assert_eq!(
        other.trouble.as_deref(),
        Some("other.txt is no longer where it was.")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn what_is_cut_is_moved_once_and_onto_nothing() {
    let (mut explorer, dir) = explorer("cutting");
    let mut fields = Fields::default();
    std::fs::write(dir.join("files/Music/notes.txt"), "theirs").unwrap();
    explorer.event("cut", &about("notes.txt"), &mut fields);
    assert!(shown(&explorer).contains("class=\"file cut\""));
    // Onto a name that is taken, it stays where it is and stays held.
    let mut other = another(&dir, "Music");
    other.event("paste", &Value::Null, &mut fields);
    assert_eq!(
        other.trouble.as_deref(),
        Some("There is already something called notes.txt here.")
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("files/Music/notes.txt")).unwrap(),
        "theirs"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("files/notes.txt")).unwrap(),
        "notes"
    );
    let mut deeper = another(&dir, "Music/Albums");
    deeper.event("paste", &Value::Null, &mut fields);
    assert_eq!(deeper.trouble, None);
    assert_eq!(names(&deeper), ["notes.txt", "one.flac"]);
    assert!(!dir.join("files/notes.txt").exists());
    // Pasted once: there is nothing held any more.
    deeper.event("paste", &Value::Null, &mut fields);
    assert_eq!(
        deeper.trouble.as_deref(),
        Some("Nothing has been cut or copied.")
    );
    // Cut and pasted where it is, it stays, and is let go.
    explorer.event("reread", &Value::Null, &mut fields);
    explorer.event("cut", &about("other.txt"), &mut fields);
    explorer.event("paste", &Value::Null, &mut fields);
    assert_eq!(
        (explorer.trouble.as_deref(), names(&explorer)),
        (None, vec!["Music", "other.txt"])
    );
    assert!(!shown(&explorer).contains(" cut\""));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn only_a_candidate_opens_a_file_and_nothing_opens_what_nothing_says_it_opens() {
    let (mut explorer, dir) = explorer("opening");
    let mut fields = Fields::default();
    // File Explorer does not open text, whatever a page says.
    explorer.event(
        "open-with",
        &said(&[
            ("name", "notes.txt"),
            ("app", "dev.peios.gxwi-file-explorer"),
        ]),
        &mut fields,
    );
    assert_eq!(
        explorer.trouble.as_deref(),
        Some("notes.txt cannot be opened with that.")
    );
    explorer.event(
        "visit",
        &said(&[("path", &dir.join("files/Music/Albums").to_string_lossy())]),
        &mut fields,
    );
    explorer.event("open", &about("one.flac"), &mut fields);
    assert_eq!(
        explorer.trouble.as_deref(),
        Some("Nothing installed says it opens audio/flac files.")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_new_name_is_typed_where_the_name_was_and_nothing_is_written_over() {
    let (mut explorer, dir) = explorer("renaming");
    let mut fields = Fields::default();
    explorer.event("rename", &about("notes.txt"), &mut fields);
    assert_eq!(fields.get("name"), "notes.txt");
    let html = shown(&explorer);
    assert!(html.contains("<form id=\"renaming-1\" class=\"file renaming\" fx-submit=\"renamed\""));
    assert!(html.contains("<input name=\"name\" fx-autofocus"));
    // Enter is the form's while a name is being typed, and Escape is the way out.
    assert!(!html.contains("fx-key=\"Enter\""));
    assert!(html.contains("fx-key=\"Escape\" fx-click=\"keep\""));
    // A name something has already is refused, and both are as they were.
    fields.set("name", "other.txt");
    explorer.event("renamed", &Value::Null, &mut fields);
    assert_eq!(
        explorer.trouble.as_deref(),
        Some("There is already something called other.txt here.")
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("files/other.txt")).unwrap(),
        "other"
    );
    assert_eq!(names(&explorer), ["Music", "notes.txt", "other.txt"]);
    // So is what is no name.
    for not_a_name in ["", "  ", "..", "a/b"] {
        explorer.event("rename", &about("notes.txt"), &mut fields);
        fields.set("name", not_a_name);
        explorer.event("renamed", &Value::Null, &mut fields);
        assert_eq!(
            explorer.trouble.as_deref(),
            Some("That is not a name a file can have."),
            "{not_a_name:?}"
        );
    }
    explorer.event("rename", &about("notes.txt"), &mut fields);
    fields.set("name", " words.txt ");
    explorer.event("renamed", &Value::Null, &mut fields);
    assert_eq!(explorer.trouble, None);
    assert_eq!(names(&explorer), ["Music", "other.txt", "words.txt"]);
    assert_eq!(explorer.picked, explorer.named("words.txt"));
    assert_eq!(
        std::fs::read_to_string(dir.join("files/words.txt")).unwrap(),
        "notes"
    );
    // Asked to rename and then not: nothing happens.
    explorer.event("rename", &about("words.txt"), &mut fields);
    explorer.event("keep", &Value::Null, &mut fields);
    fields.set("name", "never.txt");
    explorer.event("renamed", &Value::Null, &mut fields);
    assert_eq!(names(&explorer), ["Music", "other.txt", "words.txt"]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn deleting_asks_first_and_deletes_only_what_was_asked_about() {
    let (mut explorer, dir) = explorer("deleting");
    let mut fields = Fields::default();
    // Nothing is deleted that was not asked about.
    explorer.event("deleted", &about("notes.txt"), &mut fields);
    assert_eq!(names(&explorer), ["Music", "notes.txt", "other.txt"]);
    explorer.event("delete", &about("notes.txt"), &mut fields);
    let html = shown(&explorer);
    assert!(html.contains("<p>Delete notes.txt? There is nowhere to get it back from.</p>"));
    // Enter presses whichever button the keyboard is on, which starts on the way out.
    assert!(!html.contains("fx-key=\"Enter\""));
    assert!(html.contains("fx-click=\"keep\" fx-key=\"Escape\" fx-autofocus"));
    // Asked about one and told of another: neither goes.
    explorer.event("deleted", &about("other.txt"), &mut fields);
    assert_eq!(names(&explorer), ["Music", "notes.txt", "other.txt"]);
    assert!(explorer.asking.is_none());
    explorer.event("delete", &about("notes.txt"), &mut fields);
    explorer.event("keep", &Value::Null, &mut fields);
    explorer.event("deleted", &about("notes.txt"), &mut fields);
    assert_eq!(names(&explorer), ["Music", "notes.txt", "other.txt"]);
    explorer.event("delete", &about("notes.txt"), &mut fields);
    explorer.event("deleted", &about("notes.txt"), &mut fields);
    assert_eq!(names(&explorer), ["Music", "other.txt"]);
    // A directory goes with everything in it, and says so first.
    explorer.event("delete", &about("Music"), &mut fields);
    assert!(shown(&explorer).contains(
        "<p>Delete Music and everything in it? There is nowhere to get it back from.</p>"
    ));
    explorer.event("deleted", &about("Music"), &mut fields);
    assert_eq!(names(&explorer), ["other.txt"]);
    assert!(!dir.join("files/Music").exists());
    // A link is deleted as the link it is, and what it links to stays.
    std::os::unix::fs::symlink(dir.join("apps"), dir.join("files/link")).unwrap();
    explorer.event("reread", &Value::Null, &mut fields);
    explorer.event("delete", &about("link"), &mut fields);
    explorer.event("deleted", &about("link"), &mut fields);
    assert_eq!(names(&explorer), ["other.txt"]);
    assert!(dir.join("apps/dev.peios.gxwi-file-explorer.toml").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn native_names_with_the_same_display_label_keep_distinct_actions() {
    use std::os::unix::ffi::OsStringExt;
    let (mut explorer, root) = explorer("native-names");
    let native = std::ffi::OsString::from_vec(b"raw-\xff".to_vec());
    let label = native.to_string_lossy().into_owned();
    std::fs::write(explorer.at.join(&native), "native").unwrap();
    std::fs::write(explorer.at.join(&label), "utf8").unwrap();
    explorer.reread();
    let id = gxwi_fs::entry_id(&native);
    assert_ne!(explorer.named(&id), explorer.named(&label));
    assert!(shown(&explorer).contains("This path cannot be copied as UTF-8 text."));
    explorer.rename(&id, "renamed-native");
    assert_eq!(
        std::fs::read_to_string(explorer.at.join("renamed-native")).unwrap(),
        "native"
    );
    assert_eq!(
        std::fs::read_to_string(explorer.at.join(&label)).unwrap(),
        "utf8"
    );
    std::fs::write(explorer.at.join(&native), "native again").unwrap();
    explorer.reread();
    explorer.delete(&id);
    assert!(!explorer.at.join(native).exists());
    assert_eq!(
        std::fs::read_to_string(explorer.at.join(label)).unwrap(),
        "utf8"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unchanged_lossy_address_keeps_the_open_native_directory() {
    use std::os::unix::ffi::OsStringExt;
    let (mut explorer, root) = explorer("native-address");
    let native = std::ffi::OsString::from_vec(b"dir-\xff".to_vec());
    let label = native.to_string_lossy().into_owned();
    let path = explorer.at.join(&native);
    std::fs::create_dir(&path).unwrap();
    std::fs::create_dir(explorer.at.join(label)).unwrap();
    let mut fields = Fields::default();
    explorer.go(path.clone(), &mut fields);
    explorer.event("go", &Value::Null, &mut fields);
    assert_eq!(explorer.at, path);
    std::fs::remove_dir_all(root).unwrap();
}
