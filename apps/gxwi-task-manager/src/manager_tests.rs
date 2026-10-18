use super::*;
use crate::procs::{Owner, Stat};
use peinit::client::ServicePart;
use peios::token::SessionId;
use std::time::Duration;

fn stat(name: &str, ticks: u64) -> Stat {
    Stat {
        name: name.into(),
        state: 'S',
        ppid: 1,
        kernel: false,
        ticks,
        threads: 1,
        started: 0,
        resident: 100,
    }
}

fn proc(pid: u32, name: &str, service: Option<&str>, user: Option<&str>) -> Proc {
    Proc {
        pid,
        stat: Ok(stat(name, 0)),
        member: service.map(|s| CgroupMember::Service {
            service: s.into(),
            part: Some(ServicePart::Main),
        }),
        owner: match user {
            Some(sid) => Ok(Owner {
                user: sid.parse().unwrap(),
                session: SessionId(1000),
                integrity: None,
            }),
            None => Err(Closed::Refused),
        },
        psb: None,
    }
}

fn window(procs: Vec<Proc>) -> Manager {
    Manager::new(Seen {
        procs,
        ..Seen::default()
    })
}

#[test]
fn processes_are_listed_together_until_grouped() {
    let m = window(vec![
        proc(10, "sshd", Some("sshd"), Some("S-1-5-18")),
        proc(12, "bash", None, None),
    ]);
    assert_eq!(m.grouping, Grouping::None);
    let listing = m.listing(&Fields::default());
    assert!(!listing.contains("<small>"), "{listing}");
    assert!(
        listing.contains("<span class=\"name\">sshd</span>"),
        "{listing}"
    );
    assert!(
        listing.contains("<span class=\"name\">bash</span>"),
        "{listing}"
    );
}

#[test]
fn processes_group_under_their_service_and_the_rest_say_where_they_are() {
    let mut m = window(vec![
        proc(10, "sshd", Some("sshd"), Some("S-1-5-18")),
        proc(11, "sshd", Some("sshd"), Some("S-1-5-18")),
        proc(12, "bash", None, None),
    ]);
    m.grouping = Grouping::Service;
    let fields = Fields::default();
    let listing = m.listing(&fields);
    assert!(listing.contains("sshd <small>2</small>"), "{listing}");
    assert!(
        listing.contains("Not in a service <small>1</small>"),
        "{listing}"
    );
    // The person can't see whose 12 is, and is told so.
    assert!(
        m.notes().contains("You can&#39;t see who runs 1 process")
            || m.notes().contains("You can't see who runs 1 process")
    );
}

#[test]
fn by_person_a_process_whose_owner_is_closed_goes_under_someone_you_can_t_see() {
    let mut m = window(vec![
        proc(10, "sshd", Some("sshd"), Some("S-1-5-18")),
        proc(12, "bash", None, None),
    ]);
    m.grouping = Grouping::Person;
    let listing = m.listing(&Fields::default());
    assert!(listing.contains("S-1-5-18 <small>1</small>"), "{listing}");
    assert!(listing.contains("Someone you can"), "{listing}");
}

#[test]
fn a_protected_process_peinit_names_is_called_by_its_service() {
    let mut closed = proc(20, "", None, None);
    closed.stat = Err(Closed::Protected);
    closed.owner = Err(Closed::Protected);
    let mut m = window(vec![closed]);
    m.seen.mains.insert(20, "authd".into());
    m.grouping = Grouping::Service;
    let listing = m.listing(&Fields::default());
    assert!(listing.contains("authd <small>1</small>"), "{listing}");
    assert!(
        listing.contains("<span class=\"name\">authd</span>"),
        "{listing}"
    );
}

#[test]
fn the_filter_finds_by_name_pid_and_service() {
    let m = window(vec![
        proc(10, "sshd", Some("ssh"), Some("S-1-5-18")),
        proc(31, "bash", None, Some("S-1-5-18")),
    ]);
    let p10 = m.proc(10).unwrap();
    assert!(m.matches(p10, "SSHD"));
    assert!(m.matches(p10, "ssh"));
    assert!(m.matches(m.proc(31).unwrap(), "31"));
    assert!(!m.matches(p10, "bash"));
}

fn session(id: u64, sid: &str, logon_type: u32) -> LogonSessionInfo {
    LogonSessionInfo {
        id: SessionId(id),
        user: sid.parse().unwrap(),
        logon_type,
        auth_package: "lpsd".into(),
        created_at: std::time::UNIX_EPOCH + Duration::from_secs(1_791_120_598),
    }
}

#[test]
fn the_kernel_s_list_gives_each_session_its_processes_and_services_wait_to_be_asked_for() {
    let mut desktop = proc(40, "gxwi-file-explorer", None, Some("S-1-5-21-1-2-3-1002"));
    desktop.member = Some(CgroupMember::Job("4f7a".into()));
    if let Ok(owner) = &mut desktop.owner {
        owner.session = SessionId(1007);
    }
    let mut m = window(vec![desktop]);
    m.seen.sessions = Ok(vec![
        session(1007, "S-1-5-21-1-2-3-1002", 10),
        session(1004, "S-1-5-80-1-2-3-4-5", 5),
        session(999, "S-1-5-18", 5),
    ]);
    m.view = View::SignedIn;
    let listing = m.sessions_listing(&Fields::default());
    assert!(listing.contains("On the desktop"), "{listing}");
    assert!(
        !listing.contains("S-1-5-80"),
        "a service's session waits to be asked for: {listing}"
    );
    assert!(!listing.contains("S-1-5-18"), "{listing}");
    let signed = m.signed();
    assert_eq!(signed.iter().find(|s| s.id == 1007).unwrap().pids, vec![40]);
}

#[test]
fn without_the_list_the_person_s_own_sessions_are_shown_and_why_is_said() {
    let mut own = proc(50, "sh", None, Some("S-1-5-21-1-2-3-1001"));
    if let Ok(owner) = &mut own.owner {
        owner.session = SessionId(1010);
    }
    let mut m = window(vec![own]);
    m.seen.sessions = Err("Only Administrators may see everyone who is signed in.".into());
    let signed = m.signed();
    assert_eq!(signed.len(), 1);
    assert_eq!(signed[0].id, 1010);
    assert_eq!(signed[0].logon_type, None);
    assert!(m.sessions_notes().contains("Only Administrators"));
}

#[test]
fn kernel_threads_are_left_out_until_asked_for() {
    let mut k = proc(2, "kthreadd", None, Some("S-1-5-18"));
    if let Ok(stat) = &mut k.stat {
        stat.kernel = true;
    }
    let m = window(vec![k]);
    assert!(m.listing(&Fields::default()).contains("No process matches"));
}
