use super::*;
use crate::directory::{Group, User};
use auth_core::ident::{Kind, Reference};

/// The window as it would be with this directory read: nothing asks
/// authd or lpsd.
fn seen(directory: Directory) -> Manager {
    Manager {
        window: Weak::new(),
        ident: Ident::at("/nonexistent"),
        admin: Admin::at("/nonexistent"),
        view: View::Users,
        directory,
        picked: None,
        details: None,
        members: None,
        credentials: None,
        authority: Ok(()),
        doing: Doing::Looking,
        said: None,
        policy: Policy::floor(),
        may_policy: Ok(()),
        record: None,
        asked: None,
    }
}

fn sid(text: &str) -> Vec<u8> {
    text.parse::<peios::security::Sid>()
        .unwrap()
        .as_bytes()
        .to_vec()
}

fn directory() -> Directory {
    Directory {
        users: vec![
            User {
                sid: "S-1-5-21-1-2-3-1001".into(),
                name: "alice".into(),
                display_name: String::new(),
                enabled: Some(false),
                local: true,
            },
            User {
                sid: "S-1-5-21-1-2-3-1002".into(),
                name: "dana".into(),
                display_name: "Dana Scully".into(),
                enabled: Some(true),
                local: true,
            },
            User {
                sid: "S-1-5-18".into(),
                name: "SYSTEM".into(),
                display_name: String::new(),
                enabled: None,
                local: false,
            },
        ],
        groups: vec![
            Group {
                sid: "S-1-5-32-544".into(),
                name: "Administrators".into(),
                local: false,
                members: Members::Counted(1),
                description: "May change anything".into(),
            },
            Group {
                sid: "S-1-5-21-1-2-3-1100".into(),
                name: "developers".into(),
                local: true,
                members: Members::Counted(0),
                description: String::new(),
            },
            Group {
                sid: "S-1-1-0".into(),
                name: "Everyone".into(),
                local: false,
                members: Members::Rule,
                description: String::new(),
            },
        ],
        incomplete: Vec::new(),
        trouble: None,
        services: Vec::new(),
    }
}

#[test]
fn the_lists_say_who_is_local_disabled_and_how_many_are_in_a_group() {
    let mut manager = seen(directory());
    let users = manager.listing("");
    assert!(users.contains(r#"fx-value-sid="S-1-5-21-1-2-3-1001" aria-selected="false" class="off"><span class="name">alice</span><span class="full"></span><span class="kind">Local</span><span class="state">Disabled</span>"#));
    assert!(users.contains(r#"<span class="name">SYSTEM</span><span class="full"></span><span class="kind">Built-in</span>"#));
    // Found by any part of the name, the full name or the SID.
    let found = manager.listing("scully");
    assert!(found.contains("dana") && !found.contains("alice"));
    assert_eq!(
        manager.listing("nobody"),
        r#"<p class="more">Nothing matches.</p>"#
    );
    manager.view = View::Groups;
    assert!(manager.listing("").contains(r#"<span class="name">Administrators</span><span class="kind">Built-in</span><span class="members">1 member</span>"#));
}

#[test]
fn a_user_is_shown_in_full_and_their_groups_lead_to_the_group() {
    let mut manager = seen(directory());
    manager.picked = Some("S-1-5-21-1-2-3-1002".into());
    manager.details = Some(Ok(Record {
        sid: sid("S-1-5-21-1-2-3-1002"),
        qualified_name: "dana".into(),
        kind_found: Kind::Principal,
        values: vec![
            Value::UnixId(1001002),
            Value::PrimaryGroup(Reference {
                sid: sid("S-1-5-11"),
                name: "Authenticated Users".into(),
                unix_id: 101,
            }),
            Value::Home("/home/dana".into()),
            Value::DisplayName("Dana Scully".into()),
            Value::Groups(vec![Reference {
                sid: sid("S-1-5-32-544"),
                name: "Administrators".into(),
                unix_id: 102,
            }]),
            Value::Enabled(true),
            Value::LogonTypes(LogonTypes::UNSTATED),
        ],
        withheld: vec![],
    }));
    let details = manager.details(&Typed::default());
    assert!(details.contains("<h2>Dana Scully</h2><p class=\"id\">dana</p>"));
    assert!(details.contains("<dt>User ID</dt><dd>1001002</dd>"));
    assert!(details.contains(r#"<li><button type="button" class="link" fx-click="show" fx-value-sid="S-1-5-32-544">Administrators</button><button type="button" class="small" fx-click="leave" fx-value-group="S-1-5-32-544" fx-value-called="Administrators" title="Take them out of Administrators">Remove</button></li>"#), "{details}");
    assert!(details.contains("Nothing is stated, so the machine's default applies."));
    manager.show("S-1-5-32-544");
    assert_eq!(manager.view, View::Groups);
    assert_eq!(manager.picked(), Some("S-1-5-32-544"));
}

#[test]
fn a_person_who_may_not_change_the_store_is_told_why() {
    let mut manager = seen(directory());
    manager.authority = Err(format!("You may look, but not change anything. {NEEDS}"));
    manager.details = Some(Ok(Record {
        sid: sid("S-1-5-21-1-2-3-1001"),
        qualified_name: "alice".into(),
        kind_found: Kind::Principal,
        values: vec![],
        withheld: vec![],
    }));
    assert!(
        manager
            .details(&Typed::default())
            .contains("Changing users and groups needs Administrators, enabled in your token.")
    );
    manager.details = Some(Ok(Record {
        sid: sid("S-1-5-18"),
        qualified_name: "SYSTEM".into(),
        kind_found: Kind::Principal,
        values: vec![],
        withheld: vec![],
    }));
    assert!(
        manager
            .details(&Typed::default())
            .contains("Built-in: authd defines it, and it can't be changed.")
    );
    manager.details = Some(Ok(Record {
        sid: sid("S-1-5-32-544"),
        qualified_name: "Administrators".into(),
        kind_found: Kind::Group,
        values: vec![],
        withheld: vec![],
    }));
    assert!(
        manager
            .details(&Typed::default())
            .contains("who is in it is recorded on this machine")
    );
}

#[test]
fn a_local_user_may_be_changed_only_by_someone_who_may() {
    let mut manager = seen(directory());
    let dana = Record {
        sid: sid("S-1-5-21-1-2-3-1002"),
        qualified_name: "dana".into(),
        kind_found: Kind::Principal,
        values: vec![Value::Enabled(true)],
        withheld: vec![],
    };
    manager.details = Some(Ok(dana.clone()));
    let shown = manager.details(&Typed::default());
    assert!(
        shown.contains(r#"fx-click="rename""#) && shown.contains(r#"fx-click="disable""#),
        "{shown}"
    );
    manager.doing = Doing::Deleting;
    assert!(
        manager
            .details(&Typed::default())
            .contains(r#"fx-click="disable-instead""#)
    );
    manager.doing = Doing::Rename;
    assert!(
        manager
            .details(&Typed::default())
            .contains(r#"<input name="new-name""#)
    );

    manager.doing = Doing::Looking;
    manager.authority = Err(format!("You may look, but not change anything. {NEEDS}"));
    assert!(
        !manager
            .details(&Typed::default())
            .contains(r#"fx-click="rename""#)
    );
    manager.doing = Doing::Rename;
    assert!(
        !manager.details(&Typed::default()).contains("new-name"),
        "no form for someone who may not use it"
    );

    manager.doing = Doing::Looking;
    manager.authority = Ok(());
    manager.details = Some(Ok(Record {
        sid: sid("S-1-5-18"),
        qualified_name: "SYSTEM".into(),
        kind_found: Kind::Principal,
        values: vec![],
        withheld: vec![],
    }));
    assert!(
        !manager
            .details(&Typed::default())
            .contains(r#"fx-click="rename""#),
        "a built-in user is authd's"
    );
}

fn group(at: &str, name: &str, values: Vec<Value>) -> Record {
    Record {
        sid: sid(at),
        qualified_name: name.into(),
        kind_found: Kind::Group,
        values,
        withheld: vec![],
    }
}

fn member(at: &str, name: &str) -> Reference {
    Reference {
        sid: sid(at),
        name: name.into(),
        unix_id: 0,
    }
}

#[test]
fn a_local_group_may_be_changed_a_builtin_one_only_in_its_members_and_a_rule_not_at_all() {
    let mut manager = seen(directory());
    manager.details = Some(Ok(group(
        "S-1-5-21-1-2-3-1100",
        "developers",
        vec![
            Value::Members(vec![member("S-1-5-21-1-2-3-1002", "dana")]),
            Value::Description("Builds the software".into()),
        ],
    )));
    let shown = manager.details(&Typed::default());
    assert!(
        shown.contains(r#"<p class="about">Builds the software</p>"#),
        "{shown}"
    );
    for event in ["edit-group", "rename-group", "add-member", "delete-group"] {
        assert!(
            shown.contains(&format!("fx-click=\"{event}\"")),
            "{event}: {shown}"
        );
    }
    assert!(
        shown.contains(r#"fx-click="remove-member" fx-value-member="dana""#),
        "{shown}"
    );
    // Only alice may be added: dana is in it, and SYSTEM is not lpsd's.
    manager.doing = Doing::AddMember;
    assert_eq!(
        manager.choices(),
        vec![("alice".to_string(), "alice".to_string())]
    );
    manager.doing = Doing::GroupDeleting;
    assert!(manager.details(&Typed::default()).contains("has 1 member"));

    manager.doing = Doing::Looking;
    manager.details = Some(Ok(group(
        "S-1-5-32-544",
        "Administrators",
        vec![Value::Members(vec![member("S-1-5-21-1-2-3-1002", "dana")])],
    )));
    let shown = manager.details(&Typed::default());
    assert!(
        shown.contains(r#"fx-click="add-member""#) && shown.contains(r#"fx-click="remove-member""#)
    );
    assert!(
        !shown.contains(r#"fx-click="rename-group""#)
            && !shown.contains(r#"fx-click="delete-group""#)
    );

    manager.details = Some(Ok(group("S-1-1-0", "Everyone", vec![])));
    assert!(
        !manager
            .details(&Typed::default())
            .contains(r#"fx-click="add-member""#)
    );
}

#[test]
fn a_user_is_offered_the_groups_they_could_join_and_be_primary_in() {
    let mut manager = seen(directory());
    manager.details = Some(Ok(Record {
        sid: sid("S-1-5-21-1-2-3-1002"),
        qualified_name: "dana".into(),
        kind_found: Kind::Principal,
        values: vec![Value::Groups(vec![member(
            "S-1-5-32-544",
            "Administrators",
        )])],
        withheld: vec![],
    }));
    manager.doing = Doing::Join;
    assert_eq!(
        manager.choices(),
        vec![("S-1-5-21-1-2-3-1100".to_string(), "developers".to_string())]
    );
    manager.doing = Doing::Profile;
    let primaries: Vec<String> = manager
        .choices()
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    assert_eq!(
        primaries,
        ["Administrators", "developers"],
        "never a rule group such as Everyone"
    );
}

#[test]
fn an_administrator_sees_what_a_user_signs_in_with_their_keys_and_claims_to_change() {
    let mut manager = seen(directory());
    manager.picked = Some("S-1-5-21-1-2-3-1002".into());
    let claim = auth_core::claim::Claim {
        name: "Department".into(),
        flags: 0,
        values: auth_core::claim::Values::String(vec!["Engineering".into()]),
    };
    manager.details = Some(Ok(Record {
        sid: sid("S-1-5-21-1-2-3-1002"),
        qualified_name: "dana".into(),
        kind_found: Kind::Principal,
        values: vec![Value::Enabled(true), Value::Claims(vec![claim])],
        withheld: vec![],
    }));
    manager.credentials = Some(Ok((
        CredentialPolicy::PasswordOrKey,
        vec![KeyInfo {
            id: [0xab; 16],
            fingerprint: "SHA256:abc".into(),
            label: "laptop".into(),
            created: 1_791_072_000,
        }],
    )));
    let shown = manager.details(&Typed::default());
    assert!(
        shown.contains(
            "<h3>Signs in with</h3><ul class=\"plain\"><li>A password or an SSH key</li></ul>"
        ),
        "{shown}"
    );
    assert!(
        shown.contains("<span>laptop</span><code>SHA256:abc</code>"),
        "{shown}"
    );
    assert!(
        shown.contains(&format!(
            "fx-click=\"remove-key\" fx-value-key=\"{}\"",
            "ab".repeat(16)
        )),
        "{shown}"
    );
    for event in ["add-key", "add-claim", "edit-claim", "remove-claim"] {
        assert!(
            shown.contains(&format!("fx-click=\"{event}\"")),
            "{event}: {shown}"
        );
    }
    assert_eq!(
        manager.account().unwrap().policy,
        Some(CredentialPolicy::PasswordOrKey)
    );

    // Someone who may only look sees the claims, and nothing lpsd keeps.
    manager.authority = Err(format!("You may look, but not change anything. {NEEDS}"));
    let shown = manager.details(&Typed::default());
    assert!(
        shown.contains("Engineering")
            && !shown.contains("Signs in with")
            && !shown.contains("add-claim"),
        "{shown}"
    );
}

#[test]
fn a_group_says_what_its_record_grants_and_offers_one_only_to_who_may_write_it() {
    let mut manager = seen(directory());
    manager.policy = auth_policy::Policy {
        configured: true,
        records: vec![auth_policy::Record {
            privileges: Some(peios::security::Privileges::BACKUP),
            ..auth_policy::Record::empty("Administrators", "S-1-5-32-544".parse().unwrap())
        }],
        denied: peios::security::Privileges::empty(),
        problems: Vec::new(),
    };
    manager.details = Some(Ok(group("S-1-5-32-544", "Administrators", vec![])));
    let shown = manager.details(&Typed::default());
    assert!(
        shown.contains("Its record grants its members 1 privilege.")
            && shown.contains("Edit its record"),
        "{shown}"
    );
    manager.details = Some(Ok(group("S-1-5-21-1-2-3-1100", "developers", vec![])));
    assert!(
        manager
            .details(&Typed::default())
            .contains("No record: being in it grants nothing here on its own.")
    );
    manager.may_policy = Err("You may not change this machine's policy.".into());
    assert!(!manager.details(&Typed::default()).contains("own-record"));

    manager.view = View::Privileges;
    manager.record = Some("S-1-5-32-544".into());
    let record = manager.details(&Typed::default());
    assert!(
        record.contains("<code>SeBackupPrivilege</code>") && !record.contains("edit-record"),
        "{record}"
    );
}

#[test]
fn what_came_of_a_change_is_said_at_the_top_of_the_pane() {
    let mut manager = seen(directory());
    manager.said = Some(Err(
        "dana is the only principal who can administer this machine".into(),
    ));
    assert!(
        manager
            .details(&Typed::default())
            .contains(r#"<p class="note bad" role="alert">dana is the only principal"#)
    );
}

#[test]
fn a_group_s_members_by_rule_are_said_to_be_one() {
    let mut manager = seen(directory());
    manager.details = Some(Ok(Record {
        sid: sid("S-1-1-0"),
        qualified_name: "Everyone".into(),
        kind_found: Kind::Group,
        values: vec![],
        withheld: vec![auth_core::ident::Withheld {
            field: Fields::MEMBERS,
            reason: WithheldReason::Absent,
        }],
    }));
    assert!(
        manager
            .details(&Typed::default())
            .contains("Its members are decided by a rule, not listed")
    );
}
