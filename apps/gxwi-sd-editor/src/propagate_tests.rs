use super::*;
use peios::security::sddl;
use std::collections::HashMap;

/// A tree held in memory: each item's descriptor as SDDL, and what is
/// inside it.
struct Mem {
    sd: HashMap<String, Vec<u8>>,
    inside: HashMap<String, Vec<String>>,
    refuse: Option<String>,
    written: Vec<String>,
}

impl Tree for Mem {
    type Node = String;
    fn children(&mut self, node: &String) -> Result<Vec<String>, String> {
        Ok(self.inside.get(node).cloned().unwrap_or_default())
    }
    fn name(&self, node: &String) -> String {
        node.clone()
    }
    fn container(&self, node: &String) -> bool {
        self.inside.contains_key(node)
    }
    fn read(&mut self, node: &String, _: SecInfo) -> Result<Vec<u8>, String> {
        Ok(self.sd[node].clone())
    }
    fn write(&mut self, node: &String, sd: &[u8], _: SecInfo) -> Result<(), String> {
        if self.refuse.as_ref() == Some(node) {
            return Err("permission denied".into());
        }
        self.written.push(node.clone());
        self.sd.insert(node.clone(), sd.to_vec());
        Ok(())
    }
}

const FILE: Generic = Generic {
    read: 0x120089,
    write: 0x120116,
    execute: 0x1200a0,
    all: 0x1f01ff,
};

fn mem(items: &[(&str, &str, &[&str])]) -> Mem {
    Mem {
        sd: items
            .iter()
            .map(|(n, s, _)| (n.to_string(), sddl::parse(s).unwrap().as_bytes().to_vec()))
            .collect(),
        inside: items
            .iter()
            .filter(|(_, _, i)| !i.is_empty())
            .map(|(n, _, i)| (n.to_string(), i.iter().map(|s| s.to_string()).collect()))
            .collect(),
        refuse: None,
        written: vec![],
    }
}

fn text(m: &Mem, n: &str) -> String {
    sddl::format(&m.sd[n]).unwrap()
}

#[test]
fn everything_inside_gets_what_is_passed_down_but_what_protects_itself() {
    let mut m = mem(&[
        (
            "root",
            "O:BAG:BAD:(A;OICI;FA;;;BA)(A;OICIIO;GA;;;CO)",
            &["a", "locked", "file"],
        ),
        (
            "a",
            "O:S-1-5-21-1-2-3-1001G:BAD:(A;ID;FR;;;WD)",
            &["a/deep"],
        ),
        ("a/deep", "O:S-1-5-21-1-2-3-1002G:BAD:", &[]),
        ("locked", "O:BAG:BAD:P(A;;FA;;;SY)", &["locked/in"]),
        ("locked/in", "O:BAG:BAD:(A;ID;FA;;;SY)", &[]),
        ("file", "O:BAG:BAD:", &[]),
    ]);
    m.inside.insert("a/deep".into(), vec![]);
    let mut said = vec![];
    let walked = walk(
        &mut m,
        &"root".to_string(),
        &[Part::Dacl],
        FILE,
        &AtomicBool::new(false),
        &mut |n, at| said.push((n, at.to_string())),
    );
    assert_eq!(
        walked,
        Walked {
            done: 6 - 1,
            failed: vec![],
            stopped: false
        }
    );
    assert_eq!(said.len(), 5);
    // The old inherited entry is gone, CREATOR OWNER is a's owner, and
    // the rule goes on unresolved.
    assert!(
        text(&m, "a")
            .ends_with("D:AI(A;CIOIID;FA;;;BA)(A;ID;FA;;;S-1-5-21-1-2-3-1001)(A;CIOIIOID;GA;;;CO)"),
        "{}",
        text(&m, "a")
    );
    // a's inside resolves it to its own owner, from a as just written.
    assert!(
        text(&m, "a/deep")
            .ends_with("D:AI(A;CIOIID;FA;;;BA)(A;ID;FA;;;S-1-5-21-1-2-3-1002)(A;CIOIIOID;GA;;;CO)"),
        "{}",
        text(&m, "a/deep")
    );
    // What protects itself is left, and what is inside it is done from
    // it as it is.
    assert!(!m.written.contains(&"locked".to_string()));
    assert!(
        text(&m, "locked/in").ends_with("D:AI"),
        "{}",
        text(&m, "locked/in")
    );
}

#[test]
fn a_refusal_is_noted_and_the_walk_goes_on() {
    let mut m = mem(&[
        ("root", "O:BAG:BAD:(A;OICI;FA;;;BA)", &["x", "y"]),
        ("x", "O:BAG:BAD:", &[]),
        ("y", "O:BAG:BAD:", &[]),
    ]);
    m.refuse = Some("x".into());
    let walked = walk(
        &mut m,
        &"root".to_string(),
        &[Part::Dacl],
        FILE,
        &AtomicBool::new(false),
        &mut |_, _| {},
    );
    assert_eq!(walked.done, 1);
    assert_eq!(
        walked.failed,
        [Failure {
            name: "x".into(),
            why: "permission denied".into()
        }]
    );
}

#[test]
fn stopping_stops_before_the_next() {
    let mut m = mem(&[
        ("root", "O:BAG:BAD:(A;OICI;FA;;;BA)", &["x", "y"]),
        ("x", "O:BAG:BAD:", &[]),
        ("y", "O:BAG:BAD:", &[]),
    ]);
    let stop = AtomicBool::new(false);
    let walked = walk(
        &mut m,
        &"root".to_string(),
        &[Part::Dacl],
        FILE,
        &stop,
        &mut |_, _| stop.store(true, Ordering::Relaxed),
    );
    assert_eq!((walked.done, walked.stopped), (1, true));
}

#[test]
fn only_lists_that_are_passed_down_are_walked() {
    let mut m = mem(&[
        ("root", "O:BAG:BAD:(A;OICI;FA;;;BA)", &["x"]),
        ("x", "O:BAG:BAD:", &[]),
    ]);
    let walked = walk(
        &mut m,
        &"root".to_string(),
        &[Part::Owner],
        FILE,
        &AtomicBool::new(false),
        &mut |_, _| {},
    );
    assert_eq!(walked, Walked::default());
    assert!(m.written.is_empty());
}
