//! What is known about this machine's software, read from peipkg as the
//! person: what is installed, what the repositories offer, the
//! repositories themselves, the history, the roles and the roots. And
//! whether the person may change any of it, asked of the system.

use std::fs::OpenOptions;
use std::io::ErrorKind;

use crate::peipkg::{self, Offer, Operation, Package, Repository, Role, Root, Transaction};

/// Why nothing can be changed, said once.
pub const LOOK_ONLY: &str = "You can look, but you can't install or remove software: as shipped, only Administrators can.";

/// Why nothing can be seen, said once.
pub const NO_LOOK: &str = "You can't see or change this machine's software: its package records are readable only by Administrators, as shipped.";

/// The package database, which every query reads.
const DATABASE: &str = "/var/state/peipkg/db.sqlite";

/// peipkg's state: every change takes its lock and writes its database.
const STATE: [&str; 2] = ["/var/state/peipkg/lock", "/var/state/peipkg/db.sqlite"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    pub installed: Result<Vec<Package>, String>,
    pub offers: Result<Vec<Offer>, String>,
    pub repositories: Result<Vec<Repository>, String>,
    pub history: Result<Vec<Transaction>, String>,
    pub roles: Result<Vec<Role>, String>,
    pub roots: Result<Vec<Root>, String>,
    /// Whether the person may change what is installed, or why not.
    pub may: Result<(), String>,
    /// Whether the person may see what is installed at all, or why not.
    pub look: Result<(), String>,
}

/// What an update check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Updates {
    Unchecked,
    Checking,
    Found(Vec<Operation>),
    /// The check failed: peipkg's code for why, and its words.
    Failed { code: String, message: String },
}

impl Store {
    /// Nothing read yet.
    pub fn empty() -> Store {
        fn reading<T>() -> Result<T, String> {
            Err("Reading…".to_string())
        }
        Store {
            installed: reading(),
            offers: reading(),
            repositories: reading(),
            history: reading(),
            roles: reading(),
            roots: reading(),
            may: Ok(()),
            look: Ok(()),
        }
    }

    /// Why nothing on a page can be changed, or seen, if it can't, as said
    /// once under the page's heading; and whether that is all the page can
    /// show.
    pub fn banner(&self) -> (String, bool) {
        match self.look.as_ref().err().or(self.may.as_ref().err()) {
            Some(why) => (libgxwi::settings::banner(why), self.look.is_err()),
            None => (String::new(), false),
        }
    }

    pub fn package(&self, name: &str) -> Option<&Package> {
        self.installed.as_ref().ok()?.iter().find(|p| p.name == name)
    }

    /// What the repositories offer of `name`: the newest, where more than
    /// one offers it.
    pub fn offer(&self, name: &str) -> Option<&Offer> {
        let offers = self.offers.as_ref().ok()?;
        offers.iter().filter(|o| o.name == name).reduce(|a, b| if crate::words::newer(&b.version, &a.version) { b } else { a })
    }

    /// The offers not installed, one per name.
    pub fn not_installed(&self) -> Vec<&Offer> {
        let Ok(offers) = &self.offers else { return Vec::new() };
        let mut out: Vec<&Offer> = Vec::new();
        for offer in offers {
            if self.package(&offer.name).is_none() && !out.iter().any(|o| o.name == offer.name) {
                out.push(self.offer(&offer.name).unwrap_or(offer));
            }
        }
        out.sort_by(|a, b| crate::words::short(&a.name).cmp(crate::words::short(&b.name)));
        out
    }

    /// The most recent transaction that changed something and can be
    /// undone, by id.
    pub fn undoable(&self) -> Option<i64> {
        self.history.as_ref().ok()?.iter().find(|t| t.state == "committed").map(|t| t.id)
    }

    /// Whether a transaction was interrupted and waits to be resolved.
    pub fn pending(&self) -> bool {
        self.history.as_ref().is_ok_and(|h| h.iter().any(|t| t.state == "pending"))
    }
}

/// Reads everything, as the person.
pub fn read() -> Store {
    let look = look();
    if let Err(why) = &look {
        // Every query reads the database: each would fail the same way.
        fn none<T>(why: &str) -> Result<T, String> {
            Err(why.to_string())
        }
        return Store {
            installed: none(why),
            offers: none(why),
            repositories: none(why),
            history: none(why),
            roles: none(why),
            roots: none(why),
            may: none(why),
            look,
        };
    }
    Store {
        installed: peipkg::query(&["list", "--json"]).map(|mut list: Vec<Package>| {
            list.sort_by(|a, b| crate::words::short(&a.name).cmp(crate::words::short(&b.name)));
            list
        }),
        // Every package every repository offers: a search for nothing
        // matches them all.
        offers: peipkg::query(&["search", "--json", ""]),
        repositories: peipkg::query(&["repo", "list", "--json"]),
        history: peipkg::query(&["history", "--json", "-n", "50"]),
        roles: peipkg::query(&["claim", "--json"]),
        roots: peipkg::query(&["root", "list", "--json"]).map(|roots: Option<Vec<Root>>| roots.unwrap_or_default()),
        may: may(),
        look,
    }
}

/// Whether the person may read the package database. Where it isn't
/// there yet, peipkg is the one to say.
pub fn look() -> Result<(), String> {
    match std::fs::File::open(DATABASE) {
        Err(e) if e.kind() == ErrorKind::PermissionDenied => Err(NO_LOOK.into()),
        _ => Ok(()),
    }
}

/// Whether the person may change what is installed. Every change takes
/// peipkg's lock and writes its database, so may they open both to write?
/// Opening changes nothing. Where neither exists yet, peipkg is the one to
/// say.
pub fn may() -> Result<(), String> {
    for path in STATE {
        match OpenOptions::new().read(true).write(true).open(path) {
            Err(e) if e.kind() == ErrorKind::PermissionDenied => return Err(LOOK_ONLY.into()),
            _ => continue,
        }
    }
    Ok(())
}
