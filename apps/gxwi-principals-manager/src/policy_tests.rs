use super::*;
use crate::directory::{Group, Members, User};

fn sid(text: &str) -> Sid {
    text.parse().unwrap()
}

fn directory() -> Directory {
    Directory {
        users: vec![User {
            sid: "S-1-5-21-1-2-3-1002".into(),
            name: "dana".into(),
            display_name: String::new(),
            enabled: Some(true),
            local: true,
        }],
        groups: vec![Group {
            sid: "S-1-5-21-1-2-3-1100".into(),
            name: "developers".into(),
            local: true,
            members: Members::Counted(1),
            description: String::new(),
        }],
        ..Directory::default()
    }
}

fn policy() -> Policy {
    Policy {
        configured: true,
        records: vec![
            Record { privileges: Some(Privileges::CHANGE_NOTIFY), ..Record::empty("Everyone", sid("S-1-1-0")) },
            Record {
                privileges: Some(Privileges::CHANGE_NOTIFY | Privileges::BACKUP | Privileges::DEBUG),
                integrity: Some(IntegrityLevel::HIGH),
                ..Record::empty("Administrators", sid("S-1-5-32-544"))
            },
            Record { integrity: Some(IntegrityLevel::LOW), ..Record::empty("S-1-5-21-1-2-3-1100", sid("S-1-5-21-1-2-3-1100")) },
        ],
        denied: Privileges::DEBUG,
        problems: vec!["ignoring policy record Machine\\Generic\\Authn\\Policy\\developers: not a well-known principal and not a SID".into()],
    }
}

#[test]
fn the_list_names_each_record_as_a_person_would_and_says_what_authd_ignores() {
    let listed = listing(&policy(), &directory(), None, |_| true);
    assert!(
        listed.contains(
            "<span class=\"name\">Denied to everyone</span><span class=\"privs\">1 privilege</span>"
        ),
        "{listed}"
    );
    assert!(listed.contains("<span class=\"name\">Administrators</span><span class=\"privs\">3 privileges</span><span class=\"level\">High</span>"), "{listed}");
    assert!(
        listed.contains("<span class=\"name\">developers</span>"),
        "a local group's SID is named: {listed}"
    );
    let mut known = directory();
    let lpsd = auth_policy::service_sid::of("lpsd").unwrap();
    known.services.push((lpsd.to_string(), "lpsd".into()));
    assert_eq!(called(lpsd.as_ref(), &known), "The lpsd service");
    assert!(notes(&policy()).contains("authd warns: ignoring policy record"));
    assert!(notes(&Policy::floor()).contains("no policy key"));
}

#[test]
fn a_record_shows_what_it_grants_and_what_is_denied_anyway() {
    let shown = details(
        &policy(),
        &directory(),
        "S-1-5-32-544",
        &Ok(()),
        "",
        &Doing::Looking,
    );
    assert!(
        shown.contains("<code>SeDebugPrivilege</code>")
            && shown.contains("Denied to everyone, so not granted"),
        "{shown}"
    );
    assert!(
        shown.contains("fx-click=\"edit-record\"")
            && shown.contains("<dt>Integrity</dt><dd>High</dd>"),
        "{shown}"
    );
    let looking = details(
        &policy(),
        &directory(),
        "S-1-5-32-544",
        &Err("You may not change this machine's policy.".into()),
        "",
        &Doing::Looking,
    );
    assert!(
        !looking.contains("edit-record") && looking.contains("You may not change"),
        "{looking}"
    );
}

#[test]
fn what_someone_gets_is_worked_out_as_authd_would() {
    let dana = sid("S-1-5-21-1-2-3-1002");
    let shown = effective(
        &policy(),
        &directory(),
        dana.as_ref(),
        &[sid("S-1-5-32-544")],
        "interactive",
    );
    assert!(shown.contains("<dt>Integrity</dt><dd>High</dd>"), "{shown}");
    assert!(
        shown.contains("<code>SeBackupPrivilege</code>")
            && !shown.contains("<code>SeDebugPrivilege</code>"),
        "denied is taken away: {shown}"
    );
    assert!(
        shown.contains(">Everyone</button>, <button") && shown.contains(">Administrators</button>"),
        "{shown}"
    );
    // Their own record's level wins: in developers they would be Low.
    let developer = effective(
        &policy(),
        &directory(),
        sid("S-1-5-21-1-2-3-1100").as_ref(),
        &[],
        "network",
    );
    assert!(developer.contains("<dd>Low</dd>"), "{developer}");
}

#[test]
fn a_form_reads_back_as_the_draft_it_was_filled_from() {
    let mut fields = Fields::default();
    let draft = Draft {
        privileges: Some(Privileges::BACKUP | Privileges::RESTORE),
        integrity: Some(IntegrityLevel::LOW),
        owner: Some("Administrators".into()),
        default_dacl: Some("D:(A;;GA;;;SY)".into()),
    };
    fill(&draft, "", &mut fields);
    assert_eq!(super::draft(&fields, &directory()), Ok(draft));
    // A local group is named by its SID, as authd reads it.
    fields.set("owner", "developers");
    assert_eq!(
        super::draft(&fields, &directory())
            .unwrap()
            .owner
            .as_deref(),
        Some("S-1-5-21-1-2-3-1100")
    );
    fields.set("dacl", "D:(nonsense)");
    assert!(super::draft(&fields, &directory()).is_err());
}

#[test]
fn taking_passing_through_folders_from_everyone_is_asked_about_first() {
    let everyone = sid("S-1-1-0");
    assert!(risk(everyone.as_ref(), &Draft::default(), &policy()).is_some());
    assert!(
        risk(
            everyone.as_ref(),
            &Draft {
                privileges: Some(Privileges::CHANGE_NOTIFY),
                ..Draft::default()
            },
            &policy()
        )
        .is_none()
    );
    assert!(
        risk(
            sid("S-1-5-21-1-2-3-1100").as_ref(),
            &Draft::default(),
            &policy()
        )
        .is_none(),
        "only the records everyone relies on"
    );
}
