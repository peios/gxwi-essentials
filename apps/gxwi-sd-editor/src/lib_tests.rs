use super::*;

/// Taken by each test that writes an editor script and runs it. A
/// script being written is open for writing, and a child another test
/// starts meanwhile inherits that, so running the script fails with
/// ETXTBSY. One at a time, nothing is open to inherit.
static SCRIPTS: Mutex<()> = Mutex::new(());

#[cfg(feature = "splice")]
#[test]
fn only_the_parts_changed_are_put_in() {
    use peios::security::{Control, SdView, sddl};
    let bytes = |text: &str| sddl::parse(text).unwrap().as_bytes().to_vec();
    let now = bytes("O:SYG:BAD:P(A;;0xf;;;SY)");
    let edited = bytes("O:BAG:SYD:(A;;0xf;;;SY)(A;;0x1;;;WD)");
    let applied = splice(&now, &edited, &[Part::Dacl]).unwrap();
    let view = SdView::parse(&applied).unwrap();
    assert_eq!(view.owner().unwrap().to_sid().to_string(), "S-1-5-18");
    assert_eq!(view.group().unwrap().to_sid().to_string(), "S-1-5-32-544");
    assert_eq!(view.dacl().unwrap().len(), 2);
    assert!(
        !view.control().contains(Control::DACL_PROTECTED),
        "protection goes with the access list it was on"
    );
    assert_eq!(
        splice(&now, &edited, &[]).unwrap(),
        splice(&now, &now, &[Part::Dacl, Part::Owner]).unwrap()
    );
    assert!(splice(&now, b"nonsense", &[Part::Dacl]).is_err());
}

#[cfg(feature = "splice")]
#[test]
fn a_label_alone_takes_the_label_and_keeps_the_rest_of_the_sacl() {
    use peios::security::sddl;
    let bytes = |text: &str| sddl::parse(text).unwrap().as_bytes().to_vec();
    let now = bytes("O:SYG:SYD:(A;;0xf;;;SY)S:(ML;;NW;;;ME)(AU;FA;0x1;;;WD)");
    let edited = bytes("O:SYG:SYD:(A;;0xf;;;SY)S:(ML;;NWNR;;;LW)(AU;SA;0x2;;;WD)");
    let applied = sddl::format(&splice(&now, &edited, &[Part::Label]).unwrap()).unwrap();
    assert!(
        applied.contains("(AU;FA;0x1;;;WD)") && applied.contains("(ML;;NWNR;;;LW)"),
        "{applied}"
    );
    assert!(
        !applied.contains("SA;0x2") && !applied.contains(";ME)"),
        "{applied}"
    );
    // With the SACL as well, the SACL is what goes, label and all.
    let whole = sddl::format(&splice(&now, &edited, &[Part::Sacl, Part::Label]).unwrap()).unwrap();
    assert!(whole.contains("(AU;SA;0x2;;;WD)"), "{whole}");
}

fn request() -> Request {
    Request {
        object: Object {
            name: "notes.txt".into(),
            kind: "File".into(),
            ..Object::default()
        },
        sd: vec![1, 0, 4, 128],
        rights: vec![Right {
            name: "Read".into(),
            mask: 0x0012_0089,
            general: true,
        }],
        generic: Generic {
            read: 1,
            write: 2,
            execute: 4,
            all: 8,
        },
        can: Can {
            owner: true,
            ..Can::default()
        },
        ..Request::default()
    }
}

#[test]
fn what_the_redesign_adds_crosses_and_is_left_out_unsaid() {
    let mut full = request();
    full.read = Some(vec![Part::Owner, Part::Group, Part::Dacl, Part::Label]);
    full.can.label = true;
    full.object.parent = Some(Parent {
        name: "/srv".into(),
        sd: Some(vec![1, 2]),
    });
    full.object.parts = vec![ObjectPart {
        guid: "77b5b886-944a-11d1-aebd-0000f80367c1".into(),
        name: "Personal information".into(),
        kind: PartKind::Set,
        set: None,
    }];
    full.object.kinds = vec![ChildKind {
        guid: "bf967aba-0de6-11d0-a285-00aa003049e2".into(),
        name: "Accounts".into(),
    }];
    full.object.part_rights = vec![Right {
        name: "Read".into(),
        mask: 1,
        general: false,
    }];
    full.object.naming = Some(Naming {
        namespace: "e7d3a1b0-5c2f-4e8a-9b1d-0a6f3c8e2d4b".into(),
        noun: "field".into(),
        example: Some("source.name".into()),
    });
    let said = line(&full);
    assert!(
        said.contains("\"parent\":{\"name\":\"/srv\",\"sd\":\"AQI=\"}")
            && said.contains("\"read\":[\"owner\",\"group\",\"dacl\",\"label\"]"),
        "{said}"
    );
    assert!(said.contains("\"naming\":{\"namespace\":\"e7d3a1b0-5c2f-4e8a-9b1d-0a6f3c8e2d4b\",\"noun\":\"field\",\"example\":\"source.name\"}"), "{said}");
    assert_eq!(serde_json::from_str::<Request>(&said).unwrap(), full);
    let plain = line(&request());
    assert!(
        !plain.contains("parent")
            && !plain.contains("parts")
            && !plain.contains("kinds")
            && !plain.contains("naming")
            && !plain.contains("\"read\":["),
        "{plain}"
    );
    let bare: Object =
        serde_json::from_str(r#"{"name":"x","kind":"Folder","parent":{"name":"/"}}"#).unwrap();
    assert_eq!(
        bare.parent,
        Some(Parent {
            name: "/".into(),
            sd: None
        })
    );
}

#[test]
fn a_request_crosses_as_one_line_and_comes_back_the_same() {
    let said = line(&request());
    assert!(said.ends_with('\n') && said.matches('\n').count() == 1);
    assert!(said.contains("\"sd\":\"AQAEgA==\""));
    assert_eq!(serde_json::from_str::<Request>(&said).unwrap(), request());
    // A program that leaves `can` out can change the access list and
    // nothing else, as before it could say otherwise.
    let bare = r#"{"object":{"name":"k","kind":"Key"},"sd":"","rights":[],"generic":{"read":0,"write":0,"execute":0,"all":0}}"#;
    let bare: Request = serde_json::from_str(bare).unwrap();
    assert_eq!(
        (bare.object.container, &bare.can),
        (
            false,
            &Can {
                dacl: true,
                owner: false,
                audit: false,
                label: false,
                propagate: false,
                why: None
            }
        )
    );
    assert!(
        !said.contains("children"),
        "a container of everything is the default, and goes unsaid"
    );
    let key: Object = serde_json::from_str(
        r#"{"name":"sshd","kind":"Registry key","container":true,"children":"containers"}"#,
    )
    .unwrap();
    assert_eq!(key.children, Children::Containers);
    let older: Request = serde_json::from_str(
        &said
            .replace("\"can\":{", "\"can\":{\"x\":1,")
            .replace("\"dacl\":true,", ""),
    )
    .unwrap();
    assert!(older.can.dacl && older.can.owner);
    // One that may only look says so, and why.
    let looking = Can {
        dacl: false,
        why: Some("You may not change it.".into()),
        ..Can::default()
    };
    assert_eq!(
        line(&looking),
        "{\"dacl\":false,\"owner\":false,\"audit\":false,\"label\":false,\"why\":\"You may not change it.\"}\n"
    );
    assert!(!line(&Can::default()).contains("why"));
}

#[test]
fn the_answers_are_as_the_protocol_says() {
    let apply = FromEditor::Apply {
        sd: vec![0xff],
        parts: vec![Part::Owner, Part::Dacl],
        propagate: false,
    };
    assert_eq!(
        line(&apply),
        "{\"type\":\"apply\",\"sd\":\"/w==\",\"parts\":[\"owner\",\"dacl\"]}\n"
    );
    assert_eq!(line(&ToEditor::applied()), "{\"type\":\"applied\"}\n");
    // What pushing into what is inside adds is said only when there is
    // something to say, and an older answer still reads.
    let pushed = FromEditor::Apply {
        sd: vec![0xff],
        parts: vec![Part::Dacl],
        propagate: true,
    };
    assert_eq!(
        line(&pushed),
        "{\"type\":\"apply\",\"sd\":\"/w==\",\"parts\":[\"dacl\"],\"propagate\":true}\n"
    );
    assert_eq!(line(&FromEditor::Stop), "{\"type\":\"stop\"}\n");
    assert_eq!(
        line(&ToEditor::Progress {
            done: 12,
            at: "/srv/a".into()
        }),
        "{\"type\":\"progress\",\"done\":12,\"at\":\"/srv/a\"}\n"
    );
    let walked = ToEditor::Applied {
        done: 3,
        failed: vec![Failure {
            name: "/srv/b".into(),
            why: "no".into(),
        }],
        stopped: true,
    };
    assert_eq!(
        line(&walked),
        "{\"type\":\"applied\",\"done\":3,\"failed\":[{\"name\":\"/srv/b\",\"why\":\"no\"}],\"stopped\":true}\n"
    );
    assert_eq!(
        serde_json::from_str::<ToEditor>("{\"type\":\"applied\"}").unwrap(),
        ToEditor::applied()
    );
    assert!(!line(&Can::default()).contains("propagate"));
    assert_eq!(
        line(&ToEditor::Failed { why: "no".into() }),
        "{\"type\":\"failed\",\"why\":\"no\"}\n"
    );
    assert!(
        serde_json::from_str::<FromEditor>(r#"{"type":"apply","sd":"not base64!","parts":[]}"#)
            .is_err()
    );
}

#[test]
fn edit_speaks_for_the_program_until_the_editor_goes() {
    let _one = SCRIPTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // An editor that applies twice, says what it was answered, and goes.
    let dir = std::env::temp_dir().join(format!("gxwi-sd-editor-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let editor = dir.join("editor.sh");
    let heard = dir.join("heard");
    std::fs::write(
        &editor,
        format!(
            "#!/bin/sh\nread request\necho \"$request\" > {heard}\n\
             echo '{{\"type\":\"apply\",\"sd\":\"AQI=\",\"parts\":[\"dacl\"]}}'\nread answer\necho \"$answer\" >> {heard}\n\
             echo 'a line it does not know'\n\
             echo '{{\"type\":\"apply\",\"sd\":\"AQI=\",\"parts\":[\"owner\"]}}'\nread answer\necho \"$answer\" >> {heard}\n",
            heard = heard.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&editor, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let (tell, gone) = std::sync::mpsc::channel();
    let applied = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = applied.clone();
    edit_with(
        &editor,
        &request(),
        move |sd, parts| {
            seen.lock().unwrap().push((sd.to_vec(), parts.to_vec()));
            if parts == [Part::Owner] {
                Err("you are not allowed to".into())
            } else {
                Ok(())
            }
        },
        move || tell.send(()).unwrap(),
    )
    .unwrap();
    gone.recv_timeout(std::time::Duration::from_secs(10))
        .expect("done when the editor has gone");
    assert_eq!(
        *applied.lock().unwrap(),
        [
            (vec![1, 2], vec![Part::Dacl]),
            (vec![1, 2], vec![Part::Owner])
        ]
    );
    let heard = std::fs::read_to_string(&heard).unwrap();
    let heard: Vec<&str> = heard.lines().collect();
    assert_eq!(
        serde_json::from_str::<Request>(heard[0]).unwrap(),
        request()
    );
    assert_eq!(
        &heard[1..],
        [
            "{\"type\":\"applied\"}",
            "{\"type\":\"failed\",\"why\":\"you are not allowed to\"}"
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// The Editor kept raises the dialog while it is there, and says when
/// it has gone.
#[test]
fn a_raise_reaches_the_editor_while_it_is_there() {
    let _one = SCRIPTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = std::env::temp_dir().join(format!("gxwi-sd-editor-raise-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let editor = dir.join("editor.sh");
    let heard = dir.join("heard");
    std::fs::write(
        &editor,
        format!(
            "#!/bin/sh\nread request\nread raise\necho \"$raise\" > {heard}\n",
            heard = heard.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&editor, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let (tell, gone) = std::sync::mpsc::channel();
    let opened = run(
        &editor,
        &request(),
        |_, _| Ok(()),
        None,
        move || tell.send(()).unwrap(),
    )
    .unwrap();
    assert!(opened.raise());
    gone.recv_timeout(std::time::Duration::from_secs(10))
        .expect("done when the editor has gone");
    assert_eq!(
        std::fs::read_to_string(&heard).unwrap(),
        "{\"type\":\"raise\"}\n"
    );
    assert!(!opened.raise(), "nobody is there to raise");
    std::fs::remove_dir_all(dir).unwrap();
}

/// An editor that asks for what it applies to be pushed inside, and
/// says stop while the walk goes on: the walk hears it, and the answer
/// says how far it got.
#[test]
fn a_stop_is_heard_while_the_walk_goes_on() {
    let _one = SCRIPTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = std::env::temp_dir().join(format!("gxwi-sd-editor-stop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let editor = dir.join("editor.sh");
    let heard = dir.join("heard");
    std::fs::write(
        &editor,
        format!(
            "#!/bin/sh\nread request\n\
             echo '{{\"type\":\"apply\",\"sd\":\"AQI=\",\"parts\":[\"dacl\"],\"propagate\":true}}'\n\
             sleep 0.3\necho '{{\"type\":\"stop\"}}'\n\
             while read answer; do echo \"$answer\" >> {heard}; case \"$answer\" in *applied*) exit 0;; esac; done\n",
            heard = heard.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&editor, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let (tell, gone) = std::sync::mpsc::channel();
    let walk: Walk = Box::new(|parts, stop, progress| {
        assert_eq!(parts, [Part::Dacl]);
        let mut walked = Walked::default();
        while !stop.load(Ordering::Relaxed) && walked.done < 1000 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            walked.done += 1;
            progress(walked.done, "/srv/x");
        }
        walked.stopped = stop.load(Ordering::Relaxed);
        walked
    });
    run(
        &editor,
        &request(),
        |_, _| Ok(()),
        Some(walk),
        move || tell.send(()).unwrap(),
    )
    .unwrap();
    gone.recv_timeout(std::time::Duration::from_secs(10))
        .expect("done when the editor has gone");
    let heard = std::fs::read_to_string(&heard).unwrap();
    let last: ToEditor = serde_json::from_str(heard.lines().last().unwrap()).unwrap();
    assert!(
        matches!(last, ToEditor::Applied { stopped: true, done, .. } if done > 0 && done < 1000),
        "{heard}"
    );
    assert!(heard.contains("\"type\":\"progress\""), "{heard}");
    std::fs::remove_dir_all(dir).unwrap();
}
