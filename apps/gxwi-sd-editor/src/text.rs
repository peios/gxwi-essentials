//! How entries read: as SDDL, and in words.
//!
//! SDDL is written here rather than by libpeios, which refuses a whole
//! descriptor holding a type SDDL can't say. Here such a type is written as
//! a numbered comment, and the rest as libpeios writes it, so the text an
//! administrator edits is read back by libpeios (PEI-1391). The descriptor
//! goes back to the program as bytes (PSPU §8.5), so an entry that is a
//! comment still goes back exactly as it came.

use peios::security::Sid;

use crate::sd::*;
use crate::view::*;

/// The SIDs SDDL writes as two letters.
pub const ALIASES: &[(&str, &str)] = &[
    ("S-1-1-0", "WD"),
    ("S-1-3-0", "CO"),
    ("S-1-3-1", "CG"),
    ("S-1-3-4", "OW"),
    ("S-1-5-10", "PS"),
    ("S-1-5-11", "AU"),
    ("S-1-5-18", "SY"),
    ("S-1-5-19", "LS"),
    ("S-1-5-20", "NS"),
    ("S-1-5-32-544", "BA"),
    ("S-1-5-32-545", "BU"),
    ("S-1-16-4096", "LW"),
    ("S-1-16-8192", "ME"),
    ("S-1-16-12288", "HI"),
    ("S-1-16-16384", "SI"),
];

pub fn sid_text(sid: &Sid) -> String {
    let s = sid.to_string();
    ALIASES
        .iter()
        .find(|(full, _)| *full == s)
        .map_or(s, |(_, a)| a.to_string())
}

pub fn flags_text(f: u8) -> String {
    [
        (OI, "OI"),
        (CI, "CI"),
        (NP, "NP"),
        (IO, "IO"),
        (ID, "ID"),
        (SA, "SA"),
        (FA, "FA"),
    ]
    .iter()
    .filter(|(b, _)| f & b != 0)
    .map(|(_, t)| *t)
    .collect()
}

/// A mask as written: generic rights by name, a folder's Full control as
/// FA, hex otherwise.
pub fn mask_text(m: u32) -> String {
    if m != 0 && m & !GENERIC == 0 {
        return generic_name(m);
    }
    if m == 0x1f01ff {
        "FA".into()
    } else {
        format!("0x{m:x}")
    }
}

pub fn generic_name(m: u32) -> String {
    [
        (GENERIC_ALL, "GA"),
        (GENERIC_READ, "GR"),
        (GENERIC_WRITE, "GW"),
        (GENERIC_EXECUTE, "GX"),
    ]
    .iter()
    .filter(|(b, _)| m & b != 0)
    .map(|(_, t)| *t)
    .collect()
}

/// The entry's type code, or its byte where it has none.
pub fn code(ace: &Ace) -> String {
    ace.kind()
        .map_or_else(|| format!("0x{:02x}", ace.byte), |t| t.code.to_string())
}

/// One entry, as SDDL writes it.
pub fn ace_text(ace: &Ace) -> String {
    let Some(kind) = ace.kind() else {
        return format!(
            "/* type 0x{:02x}, {} bytes, kept as found */",
            ace.byte,
            ace.bytes.len()
        );
    };
    let sid = ace.sid.as_ref().map(sid_text).unwrap_or_default();
    let fl = flags_text(ace.flags);
    match ace.way {
        Way::Label => {
            let pol = if ace.mask & !7 != 0 {
                format!("0x{:x}", ace.mask)
            } else {
                [(1, "NW"), (2, "NR"), (4, "NX")]
                    .iter()
                    .filter(|(b, _)| ace.mask & b != 0)
                    .map(|(_, t)| *t)
                    .collect()
            };
            format!("(ML;{fl};{pol};;;{sid})")
        }
        Way::Trust => format!("(TL;{fl};0x{:x};;;{sid})", ace.mask),
        Way::Claim => format!(
            "(RA;{fl};;;;WD;{})",
            ace.claim.as_ref().map(|c| c.sddl()).unwrap_or_default()
        ),
        Way::Policy => format!("(SP;{fl};;;;{sid})"),
        _ => {
            let obj = ace.object.as_ref().map(guid_text).unwrap_or_default();
            let kind_guid = ace
                .inherited_object
                .as_ref()
                .map(guid_text)
                .unwrap_or_default();
            let cond = ace
                .cond
                .as_ref()
                .map(|c| format!(";({})", c.text()))
                .unwrap_or_default();
            format!(
                "({};{fl};{};{obj};{kind_guid};{sid}{cond})",
                kind.code,
                mask_text(ace.mask)
            )
        }
    }
}

/// The whole descriptor, one entry a line. An entry SDDL can't say is a
/// comment numbered within its list, which is how [`from_text`] finds it
/// again.
pub fn sd_text(sd: &Descriptor) -> String {
    let mut s = format!(
        "O:{}\nG:{}\n",
        sd.owner.as_ref().map(sid_text).unwrap_or_default(),
        sd.group.as_ref().map(sid_text).unwrap_or_default()
    );
    let flags = |p: u16, ar: u16, ai: u16| {
        [(p, "P"), (ar, "AR"), (ai, "AI")]
            .iter()
            .filter(|(bit, _)| sd.control & bit != 0)
            .map(|(_, t)| *t)
            .collect::<String>()
    };
    let entries = |s: &mut String, acl: &Acl| {
        let mut kept = 0;
        for ace in &acl.aces {
            let text = match ace.kind() {
                Some(_) => ace_text(ace),
                None => {
                    kept += 1;
                    format!(
                        "/* #{kept} kept as found: type 0x{:02x}, {} bytes */",
                        ace.byte,
                        ace.bytes.len()
                    )
                }
            };
            s.push_str(&format!("\n  {text}"));
        }
    };
    match &sd.dacl {
        Some(dacl) => {
            s.push_str(&format!("D:{}", flags(PD, DR, DI)));
            entries(&mut s, dacl);
        }
        None => s.push_str(&format!("D:{}NO_ACCESS_CONTROL", flags(PD, DR, DI))),
    }
    if let Some(sacl) = &sd.sacl {
        s.push_str(&format!("\nS:{}", flags(PS, SR_REQ, SI)));
        entries(&mut s, sacl);
    }
    s
}

/// The control bits the text says: each list's protection and inheriting.
const SAID: u16 = PD | PS | DI | SI | DR | SR_REQ;

/// What the text the person edited comes to, read by libpeios. Each entry
/// they left as it was is the very one it was, so it goes back as the bytes
/// it came as; an entry kept as found is put back where its comment stands.
/// The control bits SDDL can't say are kept from `now`.
pub fn from_text(text: &str, now: &Descriptor, ids: &mut Ids) -> Result<Descriptor, String> {
    // libpeios takes no spaces between the parts of an entry, so the layout
    // goes; inside a condition or a claim, spaces are the text's own.
    let (mut flat, mut kept, mut sections, mut entries) =
        (String::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut depth, mut quote, mut section, mut count, mut from) =
        (0usize, false, ' ', 0usize, 0usize);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quote {
            quote = c != '"';
            flat.push(c);
            continue;
        }
        if c == '(' && depth == 0 {
            from = flat.len();
        }
        match c {
            '"' => {
                quote = true;
                flat.push(c);
            }
            '/' if depth == 0 && chars.peek() == Some(&'*') => {
                chars.next();
                let mut body = String::new();
                loop {
                    match chars.next() {
                        Some('*') if chars.peek() == Some(&'/') => {
                            chars.next();
                            break;
                        }
                        Some(c) => body.push(c),
                        None => return Err("A comment isn't closed with */.".into()),
                    }
                }
                // Any other comment is the person's own, and goes.
                let number: String = body
                    .trim()
                    .strip_prefix('#')
                    .map(|r| r.chars().take_while(char::is_ascii_digit).collect())
                    .unwrap_or_default();
                if let Ok(n) = number.parse::<usize>() {
                    kept.push((section, count, n));
                }
            }
            '(' => {
                depth += 1;
                flat.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                flat.push(c);
                if depth == 0 {
                    count += 1;
                    entries.push((section, count, flat[from..].to_string()));
                }
            }
            c if c.is_whitespace() && depth <= 1 => {}
            ':' if depth == 0 => {
                section = flat.chars().last().unwrap_or(' ').to_ascii_uppercase();
                sections.push(section);
                count = 0;
                flat.push(c);
            }
            c => flat.push(c),
        }
    }
    if !sections.contains(&'D') {
        return Err(
            "Say the access list with D:, or write D:NO_ACCESS_CONTROL for none at all.".into(),
        );
    }
    if depth != 0 || quote {
        return Err(if quote {
            "A quotation isn't closed with \".".into()
        } else {
            "A bracket isn't closed.".into()
        });
    }
    // libpeios says only that it can't, so each entry is tried alone to
    // say which.
    let bytes = peios::security::sddl::parse(&flat).map_err(|_| {
        let bad = entries.iter().find(|(s, _, e)| peios::security::sddl::parse(&format!("{s}:{e}")).is_err());
        match bad {
            Some((s, n, e)) => format!("Entry {n} of the {} isn't SDDL libpeios can read: {e}", if *s == 'S' { "SACL" } else { "access list" }),
            None => "libpeios can't read this as SDDL. Check the owner and group, and the letters before each list's entries.".into(),
        }
    })?;
    let mut new = Descriptor::parse(bytes.as_bytes(), ids, &mut Found::default())?;
    new.control = (now.control & !SAID) | (new.control & SAID);
    // The entries kept as found, back where their comments are: after the
    // entries before them, and the kept ones put back before them.
    let (mut put, mut put_in) = (Vec::new(), [0usize; 2]);
    for (section, at, n) in kept {
        let (was, into, before) = match section {
            'D' => (&now.dacl, &mut new.dacl, &mut put_in[0]),
            'S' => (&now.sacl, &mut new.sacl, &mut put_in[1]),
            _ => continue,
        };
        let ace = was
            .iter()
            .flat_map(|a| &a.aces)
            .filter(|a| a.kind().is_none())
            .nth(n.wrapping_sub(1))
            .ok_or_else(|| format!("There's no entry #{n} kept as found in that list."))?;
        if put.contains(&ace.id) {
            return Err(format!("Entry #{n} kept as found is there twice."));
        }
        put.push(ace.id);
        if let Some(into) = into {
            into.aces
                .insert((at + *before).min(into.aces.len()), ace.clone());
            *before += 1;
        }
    }
    // What is as it was is what it was.
    for (was, list) in [(&now.dacl, &mut new.dacl), (&now.sacl, &mut new.sacl)] {
        let (Some(was), Some(list)) = (was, list) else {
            continue;
        };
        list.revision = was.revision;
        let mut taken = put.clone();
        for ace in list.aces.iter_mut().filter(|a| a.kind().is_some()) {
            if let Some(same) = was
                .aces
                .iter()
                .find(|w| !taken.contains(&w.id) && w.same(ace))
            {
                ace.id = same.id;
                taken.push(same.id);
            }
        }
    }
    Ok(new)
}

/// Rights in words: a general right's name where the mask is exactly one,
/// the rights it names otherwise.
pub fn rights_text(obj: &Obj, m: u32) -> String {
    if m == 0 {
        return "Nothing".into();
    }
    let (generic, rest) = (m & GENERIC, m & !GENERIC);
    let mut out = Vec::new();
    if generic != 0 {
        let as_level = obj.level_name(generic).filter(|_| rest == 0);
        out.push(generic_name(generic) + &as_level.map(|l| format!(" ({l})")).unwrap_or_default());
    }
    if rest != 0 {
        let known: Vec<(String, u32)> = obj
            .specific
            .iter()
            .map(|r| (r.name.clone(), r.mask))
            .chain([
                ("Synchronize".to_string(), SYNCHRONIZE),
                ("Read or change auditing".into(), ACCESS_SYSTEM_SECURITY),
                ("Maximum allowed".into(), MAXIMUM_ALLOWED),
            ])
            .collect();
        let names: Vec<String> = match obj.level_name(rest).filter(|_| obj.mapped(rest) == rest) {
            Some(level) => vec![level.to_string()],
            None => {
                let mut names: Vec<String> = known
                    .iter()
                    .filter(|(_, b)| rest & b == *b && *b != 0)
                    .map(|(n, _)| n.clone())
                    .collect();
                let left = rest & !known.iter().fold(0, |a, (_, b)| a | b);
                if left != 0 {
                    names.push(format!("0x{left:x}"));
                }
                names
            }
        };
        out.push(if names.len() > 3 {
            format!("{} +{}", names[..2].join(", "), names.len() - 2)
        } else {
            names.join(", ")
        });
    }
    out.join(", ")
}

/// A possessive: dana's, Contractors'.
pub fn poss(name: &str) -> String {
    if name.ends_with('s') || name.ends_with('S') {
        format!("{name}'")
    } else {
        format!("{name}'s")
    }
}

/// A list in words: a, b and c.
pub fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What the PIP types are called.
pub fn pip_type_name(t: u32) -> String {
    match t {
        0 => "None".into(),
        512 => "Protected".into(),
        1024 => "Isolated".into(),
        t => format!("Type {t}"),
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
