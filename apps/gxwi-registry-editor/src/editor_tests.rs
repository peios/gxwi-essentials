use super::*;
use peios::registry::ValueType;

/// The window as it would be with these keys read, showing `key`, with
/// `open` open: nothing asks the registry.
fn seen(key: &str, open: &[&str], reads: Vec<(&str, Result<Read, String>)>) -> Editor {
    let mut names = Names::offline();
    for (_, read) in &reads {
        if let Ok(Read {
            subkeys: Ok(subkeys),
            ..
        }) = read
        {
            for sid in subkeys.iter().filter_map(|name| name.parse::<Sid>().ok()) {
                names.learn(&sid);
            }
        }
    }
    Editor {
        window: Weak::new(),
        reads: reads
            .into_iter()
            .map(|(path, read)| (path.to_lowercase(), read))
            .collect(),
        open: open.iter().map(|path| path.to_lowercase()).collect(),
        key: key.into(),
        may: May {
            set_values: true,
            create_keys: true,
            delete: true,
            read_permissions: true,
        },
        editing: HashSet::new(),
        docs: Docs::default(),
        layers: Vec::new(),
        layer: None,
        layer_refused: None,
        held: Held {
            backup: true,
            restore: true,
        },
        choosing: false,
        value: None,
        origin: None,
        doing: Doing::Looking,
        said: None,
        names,
        zone: TimeZone::UTC,
    }
}

fn value(name: &str, data: Data) -> keys::Value {
    keys::Value {
        name: name.into(),
        size: data.encode().len(),
        sddl: keys::descriptor(&data),
        data,
    }
}

fn listing(subkeys: &[&str]) -> Result<Read, String> {
    Ok(Read {
        facts: None,
        subkeys: Ok(subkeys.iter().map(|name| name.to_string()).collect()),
        values: Ok(Vec::new()),
    })
}

fn tree(editor: &Editor) -> String {
    keys::ROOTS
        .iter()
        .map(|root| editor.node(root, root, false))
        .collect()
}

#[test]
fn a_key_reached_by_its_path_is_shown_under_a_parent_that_may_not_be_listed() {
    let reads = || {
        let closed = Ok(Read {
            facts: None,
            subkeys: Err("You may not list this key's subkeys.".into()),
            values: Ok(Vec::new()),
        });
        vec![
            ("Machine", listing(&["Closed"])),
            (r"Machine\Closed", closed),
            (r"Machine\Closed\Open", listing(&[])),
        ]
    };
    let drawn = tree(&seen(
        r"Machine\Closed\Open",
        &["Machine", r"Machine\Closed"],
        reads(),
    ));
    assert!(drawn.contains(r#"<li class="why">You may not list this key's subkeys.</li>"#));
    assert!(drawn.contains(r#"class="name reached" fx-click="show" fx-menu="key-menu" fx-value-path="Machine\Closed\Open" aria-selected="true""#));
    // Shown somewhere else, the key on the way to it is not shown there.
    let elsewhere = tree(&seen("Machine", &["Machine", r"Machine\Closed"], reads()));
    assert!(!elsewhere.contains(r#"fx-value-path="Machine\Closed\Open""#));
}

#[test]
fn a_key_with_no_subkeys_has_no_arrow_and_the_others_open_and_close() {
    let editor = seen(
        "Machine",
        &["Machine"],
        vec![
            ("Machine", listing(&["Empty", "Full"])),
            (r"Machine\Empty", listing(&[])),
        ],
    );
    let tree = tree(&editor);
    assert!(tree.contains(r#"<li role="treeitem"><div class="row"><span class="twist"></span><button type="button" class="name" fx-click="show" fx-value-path="Machine\Empty""#));
    assert!(tree.contains(r#"fx-value-path="Machine\Full" aria-label="Open Full">▸"#));
    assert!(tree.contains(r#"fx-value-path="Machine" aria-label="Close Machine">▾"#));
}

#[test]
fn a_key_under_users_says_whose_it_is() {
    let editor = seen(
        "Users",
        &["Users"],
        vec![("Users", listing(&["S-1-5-18", "Not-a-SID"]))],
    );
    let tree = tree(&editor);
    assert!(tree.contains(r#"Local System <span class="sid">S-1-5-18</span>"#));
    assert!(
        tree.contains(
            r#"fx-value-path="Users\Not-a-SID" aria-selected="false">Not-a-SID</button>"#
        )
    );
}

#[test]
fn the_default_value_comes_first_and_bytes_that_do_not_fit_are_marked() {
    let values = vec![
        value("", Data::Sz("first".into())),
        value("Broken", Data::Raw(ValueType::DWORD, vec![1, 2, 3])),
    ];
    let mut editor = seen(
        "Machine",
        &[],
        vec![(
            "Machine",
            Ok(Read {
                facts: None,
                subkeys: Ok(Vec::new()),
                values: Ok(values),
            }),
        )],
    );
    let listing = editor.listing();
    assert!(listing.find("(Default)").unwrap() < listing.find("Broken").unwrap());
    assert!(listing.contains(r#"<span class="data misfit">01 02 03</span>"#));
    editor.value = Some("Broken".into());
    let details = editor.details();
    assert!(details.contains("Number (REG_DWORD)"));
    assert!(details.contains("which is 4 bytes, but it holds 3 bytes"));
    assert!(details.contains(r#"<span class="at">0000</span>  01 02 03"#));
}

#[test]
fn what_may_not_be_changed_is_offered_disabled_and_said_in_words() {
    let one = || vec![value("Theme", Data::Sz("dark".into()))];
    let read = || {
        Ok(Read {
            facts: None,
            subkeys: Ok(Vec::new()),
            values: Ok(one()),
        })
    };
    let mut editor = seen(r"Machine\App", &[], vec![(r"Machine\App", read())]);
    editor.may = May {
        set_values: false,
        create_keys: false,
        delete: false,
        read_permissions: true,
    };
    let details = editor.details();
    assert!(details.contains(
        r#"fx-click="new-key" disabled title="You may not create keys under this key.""#
    ));
    assert!(details.contains(r#"fx-click="delete-key" disabled"#));
    assert!(details.contains(r#"<p class="may">You may read this key but not change it.</p>"#));
    // Nothing to edit with: no double-click on the rows.
    assert!(!editor.listing().contains("fx-dblclick"));
    editor.may.create_keys = true;
    assert!(
        editor
            .details()
            .contains("You may not change its values or delete it.")
    );
    editor.value = Some("Theme".into());
    assert!(
        editor
            .details()
            .contains(r#"fx-click="edit" fx-value-name="Theme" disabled"#)
    );
    // A root may never be deleted, so it isn't offered.
    let root = seen("Machine", &[], vec![("Machine", read())]);
    assert!(!root.details().contains("delete-key"));
}

#[test]
fn a_value_is_edited_in_the_form_for_its_type() {
    let values = vec![
        value("Count", Data::Dword(7)),
        value("Broken", Data::Raw(ValueType::SZ, vec![0xff])),
    ];
    let mut editor = seen(
        "Machine",
        &[],
        vec![(
            "Machine",
            Ok(Read {
                facts: None,
                subkeys: Ok(Vec::new()),
                values: Ok(values),
            }),
        )],
    );
    let mut fields = Fields::default();
    editor.edit(Some("Count".into()), &mut fields);
    assert_eq!(fields.get("value-data"), "7");
    let form = editor.details();
    assert!(form.contains(r#"name="value-base""#) && form.contains("from 0 to 4294967295"));
    // Said in hex, the number is written again in hex.
    fields.set("value-base", "hex");
    editor.input("value-base", &mut fields);
    assert_eq!(fields.get("value-data"), "0x7");
    assert!(editor.details().contains("from 0 to 0xffffffff"));
    editor.edit(Some("Broken".into()), &mut fields);
    assert_eq!(fields.get("value-data"), "ff");
    assert!(editor.details().contains("so it is edited as bytes"));
    // A new value's form changes with the type picked.
    editor.edit(None, &mut fields);
    assert!(editor.details().contains(r#"name="value-name""#));
    fields.set("value-type", &ValueType::MULTI_SZ.0.to_string());
    editor.input("value-type", &mut fields);
    assert!(editor.details().contains("One item to a line."));
    // One of the types not offered for a new value is not taken.
    fields.set("value-type", &ValueType::LINK.0.to_string());
    editor.input("value-type", &mut fields);
    assert!(editor.details().contains("One item to a line."));
}

#[test]
fn a_new_value_may_not_take_a_name_already_used() {
    let values = vec![value("Theme", Data::Sz("dark".into()))];
    let mut editor = seen(
        "Machine",
        &[],
        vec![(
            "Machine",
            Ok(Read {
                facts: None,
                subkeys: Ok(Vec::new()),
                values: Ok(values),
            }),
        )],
    );
    let mut fields = Fields::default();
    editor.edit(None, &mut fields);
    fields.set("value-name", "Theme");
    fields.set("value-data", "light");
    editor.save(&mut fields);
    assert_eq!(
        editor.said,
        Said::bad("There is a value called Theme already. Edit it instead.")
    );
    assert!(matches!(editor.doing, Doing::Editing { .. }));
}

#[test]
fn a_value_holding_a_descriptor_is_shown_as_one_and_opens_in_the_editor() {
    let sd = peios::security::sddl::parse("O:BAG:BAD:(A;;GA;;;SY)(A;;GR;;;WD)").unwrap();
    let values = vec![
        value("Guard", Data::Binary(sd.as_bytes().to_vec())),
        value("Plain", Data::Binary(vec![1, 0, 0, 0x80])),
    ];
    let mut editor = seen(
        "Machine",
        &[],
        vec![(
            "Machine",
            Ok(Read {
                facts: None,
                subkeys: Ok(Vec::new()),
                values: Ok(values),
            }),
        )],
    );
    let listing = editor.listing();
    assert!(
        listing.contains(
            r#"<span class="type">Security descriptor</span><span class="data">O:BAG:BAD:"#
        )
    );
    // Bytes that start as one does but don't parse are only bytes.
    assert!(
        listing.contains(r#"<span class="type">Bytes</span><span class="data">01 00 00 80</span>"#)
    );
    editor.value = Some("Guard".into());
    let details = editor.details();
    assert!(details.contains(r#"fx-click="permissions" fx-value-name="Guard""#));
    assert!(details.contains("<h3>Security descriptor</h3>"));
    // Who it lets in may be looked at by someone who can't change it.
    editor.may.set_values = false;
    assert!(
        editor
            .details()
            .contains(r#"fx-click="permissions" fx-value-name="Guard">"#)
    );
    editor.value = Some("Plain".into());
    assert!(!editor.details().contains(r#"fx-click="permissions""#));
}

#[test]
fn what_the_manual_says_is_shown_and_a_documented_value_can_be_set_from_it() {
    let (records, _) = libregman::fragment::parse(
        "--- machine\\app\ncanonical: Machine\\App\n\nThe app's **settings**.\n\n\
         --- machine\\app theme\ncanonical: Machine\\App Theme\ntype: REG_SZ\ndefault: dark\napplies: live\n\nIts colours.\n\n\
         --- machine\\app size\ncanonical: Machine\\App Size\ntype: REG_DWORD\ndefault: 12\n\nIts size.\n",
    );
    let values = vec![value("Theme", Data::Dword(1))];
    let mut editor = seen(
        r"Machine\App",
        &[],
        vec![(
            r"Machine\App",
            Ok(Read {
                facts: None,
                subkeys: Ok(Vec::new()),
                values: Ok(values),
            }),
        )],
    );
    editor.docs = Docs {
        key: Some(records[0].clone()),
        values: vec![
            ("Size".into(), records[2].clone()),
            ("Theme".into(), records[1].clone()),
        ],
    };
    let key = editor.details();
    assert!(key.contains("<h3>About this key</h3><div class=\"prose\"><p>The app's <strong>settings</strong>.</p></div>"));
    // Theme is set; Size isn't, and is listed after it, greyed, saying
    // so, and set by double-clicking it.
    let listing = editor.listing();
    assert!(listing.find(">Theme<").unwrap() < listing.find(">Size<").unwrap());
    assert!(listing.contains(r#"<li class="unset"><button type="button" fx-click="value" fx-dblclick="set-documented" fx-value-name="Size" aria-selected="false""#));
    assert!(listing.contains(
        r#"<span class="type">Number</span><span class="data">Not set · default 12</span>"#
    ));
    assert!(!listing.contains(r#"class="unset"><button type="button" fx-click="value" fx-dblclick="set-documented" fx-value-name="Theme""#));
    // It isn't a value: not counted, and not in the key's pane.
    assert!(editor.footer().contains("1 value<"));
    assert!(!key.contains("Size"));
    editor.value = Some("Theme".into());
    let theme = editor.details();
    assert!(theme.contains("<dt>A change applies</dt><dd>At once</dd>"));
    assert!(theme.contains("The manual says it should be REG_SZ, but it is REG_DWORD"));
    // Picked, it shows what the manual says, and can be set.
    editor.pick(Some("Size".into()));
    let unset = editor.details();
    assert!(unset.contains("It isn't set here, so whatever reads it uses its default."));
    assert!(unset.contains(
        r#"<button type="button" fx-click="set-documented" fx-value-name="Size">Set…</button>"#
    ));
    assert!(unset.contains("<dt>Default</dt><dd>12</dd>") && !unset.contains("but it is"));
    let mut fields = Fields::default();
    editor.event(
        "set-documented",
        &serde_json::json!({ "name": "Size" }),
        &mut fields,
    );
    assert_eq!(fields.get("value-name"), "Size");
    assert_eq!(fields.get("value-type"), ValueType::DWORD.0.to_string());
    assert_eq!(fields.get("value-data"), "12");
    assert!(matches!(
        editor.doing,
        Doing::Editing {
            name: None,
            ty: ValueType::DWORD,
            documented: true,
            ..
        }
    ));
    // Its name is given, so its data is what is typed first.
    let form = editor.details();
    assert!(
        form.contains(
            r#"<input name="value-name" autocomplete="off" spellcheck="false" placeholder"#
        )
    );
    assert!(form.contains(r#"<input name="value-data" inputmode="numeric" autocomplete="off" spellcheck="false" fx-autofocus>"#));
    // Once it is set, it is a value like the others.
    editor.reads.insert(
        r"machine\app".into(),
        Ok(Read {
            facts: None,
            subkeys: Ok(Vec::new()),
            values: Ok(vec![
                value("Size", Data::Dword(12)),
                value("Theme", Data::Dword(1)),
            ]),
        }),
    );
    assert!(!editor.listing().contains("class=\"unset\""));
    // Without the right to change values, it can't be set from here.
    editor.reads.insert(
        r"machine\app".into(),
        Ok(Read {
            facts: None,
            subkeys: Ok(Vec::new()),
            values: Ok(Vec::new()),
        }),
    );
    editor.may.set_values = false;
    editor.doing = Doing::Looking;
    assert!(!editor.listing().contains("fx-dblclick"));
    editor.pick(Some("Size".into()));
    assert!(editor.details().contains(r#"fx-click="set-documented" fx-value-name="Size" disabled title="You may not change this key's values.""#));
}

#[test]
fn a_key_with_only_documented_values_lists_them_and_one_with_none_says_so() {
    let (records, _) = libregman::fragment::parse(
        "--- machine\\app mode\ncanonical: Machine\\App Mode\ntype: REG_SZ\ndefault: (none)\n\nIts mode.\n",
    );
    let mut editor = seen(r"Machine\App", &[], vec![(r"Machine\App", listing(&[]))]);
    assert_eq!(
        editor.listing(),
        r#"<p class="more">This key has no values.</p>"#
    );
    editor.docs = Docs {
        key: None,
        values: vec![("Mode".into(), records[0].clone())],
    };
    assert!(
        editor
            .listing()
            .contains(r#"<span class="type">Text</span><span class="data">Not set</span>"#)
    );
    assert!(
        editor
            .details()
            .contains("The registry manual documents values of this key, but not the key itself.")
    );
    // A default that is only a remark isn't typed in.
    let mut fields = Fields::default();
    editor.event(
        "set-documented",
        &serde_json::json!({ "name": "Mode" }),
        &mut fields,
    );
    assert_eq!(fields.get("value-data"), "");
}

#[test]
fn a_layer_that_may_not_be_written_makes_the_key_read_only_and_says_why() {
    let mut editor = seen(
        r"Machine\App",
        &[],
        vec![(
            r"Machine\App",
            Ok(Read {
                facts: None,
                subkeys: Ok(Vec::new()),
                values: Ok(vec![value("Theme", Data::Sz("dark".into()))]),
            }),
        )],
    );
    editor.layer_refused = Some("You may not write into the layer policy.".into());
    editor.may.set_values = false;
    let details = editor.details();
    assert!(details.contains(
        r#"fx-click="new-value" disabled title="You may not write into the layer policy.""#
    ));
    assert!(details.contains(
        "You may not write into the layer policy. Choose another layer to change this key."
    ));
}

#[test]
fn with_layers_the_value_says_only_the_winner_is_shown_and_where_changes_go() {
    let layer = |name: &str, precedence: u32| Layer {
        name: name.into(),
        precedence,
        enabled: true,
        owner: None,
        malformed: false,
    };
    let mut editor = seen(
        "Machine",
        &[],
        vec![(
            "Machine",
            Ok(Read {
                facts: None,
                subkeys: Ok(Vec::new()),
                values: Ok(vec![value("Theme", Data::Sz("dark".into()))]),
            }),
        )],
    );
    editor.value = Some("Theme".into());
    editor.origin = Some(Origin {
        layer: "policy".into(),
        sequence: 7,
    });
    assert_eq!(editor.layer_note(), "");
    editor.layers = vec![layer("policy", 100), layer(BASE, 0)];
    let note = editor.layer_note();
    assert!(note.contains("Only the entry that wins is shown."));
    assert!(note.contains("Changes go to the layer base, beneath policy, so they won't show while policy's entry is there."));
    let mut fields = Fields::default();
    fields.set("layer", "policy");
    editor.input("layer", &mut fields);
    assert_eq!(editor.layer.as_deref(), Some("policy"));
    assert!(!editor.layer_note().contains("Changes go to"));
    assert!(
        editor
            .layer_options()
            .contains(r#"<option value="policy">policy</option>"#)
    );
}

#[test]
fn files_are_offered_as_far_as_the_privileges_and_the_layer_allow() {
    let mut editor = seen(r"Machine\App", &[], vec![(r"Machine\App", listing(&[]))]);
    let buttons = editor.file_buttons();
    assert!(
        buttons.contains(r#"fx-click="export">Export…"#)
            && buttons.contains(r#"fx-click="restore">Restore…"#)
    );
    editor.held = Held {
        backup: false,
        restore: false,
    };
    let buttons = editor.file_buttons();
    assert!(buttons.contains(r#"fx-click="backup" disabled title="Backing up needs the privilege to back up files and keys (SeBackupPrivilege)"#));
    assert!(
        buttons.contains(r#"fx-click="restore" disabled title="Restoring needs the privilege"#)
    );
    editor.layer_refused = Some("You may not write into the layer policy.".into());
    assert!(editor.file_buttons().contains(
        r#"fx-click="import" disabled title="You may not write into the layer policy.""#
    ));
    editor.choosing = true;
    assert!(
        editor
            .file_buttons()
            .contains(r#"fx-click="export" disabled title="A file is being chosen already.""#)
    );
}

#[test]
fn an_import_says_where_it_writes_before_it_does() {
    let mut editor = seen(r"Machine\App", &[], vec![(r"Machine\App", listing(&[]))]);
    let doc: Document =
        serde_json::from_str(r#"{"keys":[{"path":"Machine\\App"},{"path":"Machine\\App\\Sub"}]}"#)
            .unwrap();
    editor.doing = Doing::Importing {
        doc,
        file: "/home/dana/App.json".into(),
    };
    let asked = editor.details();
    assert!(
        asked.contains(r"Write <strong>Machine\App and 1 more key</strong>, with their values?")
    );
    editor.layer = Some("policy".into());
    assert!(editor.details().contains(", into the layer policy"));
    editor.doing = Doing::Restoring("/home/dana/App.regbackup".into());
    assert!(editor.details().contains(
        r"Replace <strong>Machine\App</strong> and everything under it with what the backup holds?"
    ));
}

#[test]
fn the_key_s_permissions_are_offered_where_they_may_be_read() {
    let mut editor = seen("Machine", &[], vec![("Machine", listing(&[]))]);
    assert!(
        editor
            .details()
            .contains(r#"<button type="button" fx-click="permissions">Permissions…</button>"#)
    );
    editor.may.read_permissions = false;
    assert!(editor.details().contains(
        r#"fx-click="permissions" disabled title="You may not read who may use this key.""#
    ));
}

#[test]
fn deleting_asks_first() {
    let values = vec![value("Theme", Data::Sz("dark".into()))];
    let mut editor = seen(
        r"Machine\App",
        &[],
        vec![(
            r"Machine\App",
            Ok(Read {
                facts: None,
                subkeys: Ok(vec!["A".into(), "B".into()]),
                values: Ok(values),
            }),
        )],
    );
    let mut fields = Fields::default();
    editor.event(
        "delete-value",
        &serde_json::json!({ "name": "Theme" }),
        &mut fields,
    );
    assert!(
        editor
            .details()
            .contains("Delete the value <strong>Theme</strong>? This can't be undone.")
    );
    editor.event("cancel", &serde_json::json!({}), &mut fields);
    assert_eq!(editor.doing, Doing::Looking);
    editor.value = None;
    editor.event("delete-key", &serde_json::json!({}), &mut fields);
    assert!(editor.details().contains("Delete <strong>App</strong>, the 2 keys under it and everything under those, with all their values? This can't be undone."));
}

#[test]
fn a_key_that_may_not_be_read_says_so_where_its_values_would_be() {
    let editor = seen(
        "Machine",
        &[],
        vec![("Machine", Err("You may not read this key.".into()))],
    );
    assert_eq!(
        editor.listing(),
        r#"<p class="trouble">You may not read this key.</p>"#
    );
    assert!(
        editor
            .details()
            .contains(r#"<p class="note bad">You may not read this key.</p>"#)
    );
}
