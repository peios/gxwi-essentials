use super::*;
use crate::view::tests::{folder, read};
use peios::security::sddl;

#[test]
fn entries_read_as_sddl_writes_them() {
    for text in [
        "(A;OICI;FA;;;SY)",
        "(D;OICI;0x10156;;;S-1-5-21-1-2-3-1106)",
        "(XA;OICI;0x1200a9;;;AU;(@User.Department == @Resource.Department))",
        "(OA;CIIO;0x100;00299570-246d-11d0-a768-00aa006e0529;bf967aba-0de6-11d0-a285-00aa003049e2;S-1-5-21-1-2-3-1000)",
        "(A;OICIIO;GA;;;CO)",
    ] {
        let (sd, _, _) = read(&format!("O:SYG:SYD:{text}"));
        assert_eq!(ace_text(&sd.dacl.unwrap().aces[0]), text);
    }
    let (sd, found, _) =
        read("O:SYG:SYD:(A;;0x1;;;WD)S:(ML;;NWNR;;;LW)(RA;;;;;WD;(\"Budget\",TU,0x20,250000))");
    let text = sd_text(&sd);
    assert!(
        text.contains("(ML;;NWNR;;;LW)")
            && text.contains("(RA;;;;;WD;(\"Budget\",TU,0x20,250000))"),
        "{text}"
    );
    // What it writes, libpeios reads as the same descriptor.
    let flat = text.replace("\n  ", "").replace('\n', "");
    assert_eq!(
        sddl::format(sddl::parse(&flat).unwrap().as_bytes()).unwrap(),
        sddl::format(&sd.build(&found).unwrap()).unwrap()
    );
}

/// What the descriptor read from `sddl` comes to with its text edited
/// by `edit`, with the bytes it is then written as.
fn edited(
    sddl: &str,
    edit: impl Fn(String) -> String,
) -> Result<(Descriptor, Descriptor, Vec<u8>), String> {
    let (sd, found, mut ids) = read(sddl);
    let new = from_text(&edit(sd_text(&sd)), &sd, &mut ids)?;
    let bytes = new.build(&found)?;
    Ok((sd, new, bytes))
}

#[test]
fn text_left_alone_is_the_descriptor_it_was() {
    for sddl in [
        "O:SYG:SYD:PAI(A;OICI;FA;;;SY)(XA;OICI;0x1200a9;;;AU;(@User.Department == @Resource.Department))(A;OICIID;0x1301bf;;;BA)S:(ML;;NW;;;ME)(RA;;;;;WD;(\"Budget\",TU,0x20,250000))(AL;SA;0x10000;;;WD)(TL;;0x20000;;;S-1-19-512-1024)",
        "O:BAG:BAD:(OA;CIIO;0x100;00299570-246d-11d0-a768-00aa006e0529;bf967aba-0de6-11d0-a285-00aa003049e2;S-1-5-21-1-2-3-1000)",
    ] {
        let (sd, new, _) = edited(sddl, |t| t).unwrap();
        assert_eq!(new, sd, "{sddl}");
    }
}

#[test]
fn only_what_the_text_changes_is_new() {
    let (sd, new, bytes) = edited(
        "O:SYG:SYD:(A;;FA;;;SY)(XA;;0x1200a9;;;AU;(@User.X == 1))(A;;0x1;;;WD)",
        |t| t.replace("(A;;0x1;;;WD)", "(D;;0x1;;;WD)"),
    )
    .unwrap();
    let (was, now) = (
        &sd.dacl.as_ref().unwrap().aces,
        &new.dacl.as_ref().unwrap().aces,
    );
    assert_eq!((now[0].id, now[1].id), (was[0].id, was[1].id));
    assert_ne!(now[2].id, was[2].id);
    assert_eq!(now[2].way, Way::Deny);
    assert!(sddl::format(&bytes).unwrap().ends_with("(D;;0x1;;;WD)"));
}

#[test]
fn an_entry_kept_as_found_stays_where_its_comment_is() {
    let (mut sd, found, mut ids) = read("O:SYG:SYD:(A;;FA;;;SY)(A;;0x1;;;WD)");
    // An entry of a type nobody has, between the two.
    let odd = crate::sd::parse_ace(
        &[0x16, 0, 12, 0, 1, 0, 0, 0, 0xaa, 0xbb, 0xcc, 0xdd],
        ids.next(),
    )
    .unwrap();
    sd.dacl.as_mut().unwrap().aces.insert(1, odd.clone());
    let text = sd_text(&sd);
    assert!(
        text.contains("/* #1 kept as found: type 0x16, 12 bytes */"),
        "{text}"
    );
    let moved = text
        .replace("  /* #1 kept as found: type 0x16, 12 bytes */\n", "")
        .replace("(A;;0x1;;;WD)", "(A;;0x1;;;WD)\n/* #1 */");
    let new = from_text(&moved, &sd, &mut ids).unwrap();
    let aces = &new.dacl.as_ref().unwrap().aces;
    assert_eq!(
        aces.iter().map(|a| a.way).collect::<Vec<_>>(),
        [Way::Allow, Way::Allow, Way::Unknown]
    );
    assert_eq!(aces[2].bytes, odd.bytes);
    assert!(
        new.build(&found)
            .unwrap()
            .windows(12)
            .any(|w| w == odd.bytes)
    );
    // Taken out, it is gone.
    let gone = from_text(
        &text.replace("/* #1 kept as found: type 0x16, 12 bytes */", ""),
        &sd,
        &mut ids,
    )
    .unwrap();
    assert_eq!(gone.dacl.unwrap().aces.len(), 2);
    assert!(
        from_text(&format!("{text}\n/* #1 */"), &sd, &mut ids)
            .unwrap_err()
            .contains("twice")
    );
    assert!(
        from_text(&text.replace("#1 kept", "#4 kept"), &sd, &mut ids)
            .unwrap_err()
            .contains("#4")
    );
}

#[test]
fn text_says_no_access_list_and_the_flags_it_has_words_for() {
    let (sd, new, _) = edited("O:SYG:SYD:PAI(A;;FA;;;SY)", |t| {
        t.replace("D:PAI\n  (A;;FA;;;SY)", "D:NO_ACCESS_CONTROL")
    })
    .unwrap();
    assert!(new.dacl.is_none() && sd.dacl.is_some());
    assert_eq!(new.control & PD, 0);
    let (_, new, _) = edited("O:SYG:SYD:(A;;FA;;;SY)", |t| t.replace("D:", "D:PAR")).unwrap();
    assert_eq!(new.control & (PD | DR), PD | DR);
    assert!(sd_text(&new).contains("D:PAR\n"));
}

#[test]
fn what_is_wrong_with_the_text_is_said_where_it_is() {
    let err = edited("O:SYG:SYD:(A;;FA;;;SY)(A;;0x1;;;WD)", |t| {
        t.replace("(A;;0x1;;;WD)", "(QQ;;0x1;;;WD)")
    })
    .unwrap_err();
    assert_eq!(
        err,
        "Entry 2 of the access list isn't SDDL libpeios can read: (QQ;;0x1;;;WD)"
    );
    assert!(
        edited("O:SYG:SYD:(A;;FA;;;SY)", |t| t.replace("D:", ""))
            .unwrap_err()
            .contains("D:NO_ACCESS_CONTROL")
    );
    assert!(
        edited("O:SYG:SYD:(A;;FA;;;SY)", |t| t.replace(";SY)", ";SY"))
            .unwrap_err()
            .contains("bracket")
    );
    assert!(
        edited("O:SYG:SYD:(A;;FA;;;SY)", |t| format!("{t} /* not closed"))
            .unwrap_err()
            .contains("*/")
    );
}

#[test]
fn null_and_empty_access_lists_round_trip_with_flags() {
    for sddl in [
        "O:SYG:SYD:NO_ACCESS_CONTROL",
        "O:SYG:SYD:PAINO_ACCESS_CONTROL",
        "O:SYG:SYD:",
    ] {
        let (sd, new, _) = edited(sddl, |text| text).unwrap();
        assert_eq!(new.dacl.is_none(), sd.dacl.is_none());
        assert_eq!(new.control & SAID, sd.control & SAID);
        assert_eq!(sd_text(&new), sd_text(&sd));
    }
    assert!(
        edited("O:SYG:SYD:(A;;FA;;;SY)", |text| text
            .replace("D:", "D:NO_ACCESS_CONTROL"))
        .is_err()
    );
}

#[test]
fn rights_read_as_their_names() {
    let f = folder();
    assert_eq!(rights_text(&f, 0x1301bf), "Modify");
    assert_eq!(rights_text(&f, GENERIC_ALL), "GA (Full control)");
    assert_eq!(
        rights_text(&f, 0x3),
        "List folder / read data, Create files / write data"
    );
    assert_eq!(
        and_list(&["a".into(), "b".into(), "c".into()]),
        "a, b and c"
    );
}
