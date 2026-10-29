//! What the window shows of the trust store, read in one look outside its
//! lock: the certificates in force, how trustd is faring, what is
//! distrusted, and what this person may change.
//!
//! The certificates come from trustd, the only one that knows the set in
//! force. What is distrusted comes from the registry, since a distrusted
//! certificate is by definition not in the store; its name is found in the
//! bundle Peios ships, or among this machine's additions, where it is one.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::sync::{Arc, Weak};
use std::time::Duration;

use gxwi::Surface;
use peios::registry::{Key, KeyAccess, OpenFlags};
use trust_client::{
    ADD_KEY, CERTIFICATE_VALUE, CERTIFICATES_KEY, Client, Details, Root, SHARE_BUNDLE, Status,
};

use crate::policy::Policy;

/// The reason, said once, why someone may look but not change.
pub const LOOK_ONLY: &str = "You can look, but you can't change what this machine trusts: as shipped, only Administrators can.";

/// One certificate in force, and what it says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cert {
    pub root: Root,
    pub details: Details,
}

impl Cert {
    pub fn name(&self) -> &str {
        self.details.name()
    }

    /// Its organisation, where that says more than its name does.
    pub fn organisation(&self) -> Option<&str> {
        self.details
            .organisation
            .as_deref()
            .filter(|o| *o != self.name())
    }

    /// Whether `words` (already lower case) is found in anything shown of it.
    pub fn matches(&self, words: &str) -> bool {
        let found = |text: &str| text.to_lowercase().contains(words);
        found(self.name())
            || self.details.organisation.as_deref().is_some_and(found)
            || self.details.unit.as_deref().is_some_and(found)
            || found(&self.details.subject)
            || self.root.name.as_deref().is_some_and(found)
            || trust_client::normalise_fingerprint(words)
                .is_ok_and(|typed| self.root.fingerprint.starts_with(&typed))
    }
}

/// One distrusted certificate, named where it can be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub fingerprint: String,
    pub reason: String,
    /// What it is called and who issued it, where this machine has a copy.
    pub known: Option<Details>,
}

/// Everything shown, as last read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    /// The certificates in force, or why they couldn't be had.
    pub roots: Result<Vec<Cert>, String>,
    pub status: Option<Status>,
    pub distrusted: Result<Vec<Refused>, String>,
    /// Whether this person may add, remove, distrust and restore.
    pub may: Result<(), String>,
    /// Whether they may turn the files under /etc/ssl on and off.
    pub may_compat: Result<(), String>,
    /// Whether trustd would let them ask it to compose again.
    pub may_reload: bool,
    /// The keys the kernel verifies signatures with.
    pub keys: Result<Vec<crate::keys::SigningKey>, crate::keys::Unlisted>,
}

impl Store {
    pub fn certificate(&self, fingerprint: &str) -> Option<&Cert> {
        self.roots
            .as_ref()
            .ok()?
            .iter()
            .find(|c| c.root.fingerprint == fingerprint)
    }

    pub fn refused(&self, fingerprint: &str) -> Option<&Refused> {
        self.distrusted
            .as_ref()
            .ok()?
            .iter()
            .find(|r| r.fingerprint == fingerprint)
    }

    /// The additions' names, to refuse a second under one of them.
    pub fn added_names(&self) -> Vec<String> {
        self.roots
            .as_ref()
            .map(|roots| roots.iter().filter_map(|c| c.root.name.clone()).collect())
            .unwrap_or_default()
    }
}

/// Certificates this machine has copies of, by fingerprint, for naming the
/// distrusted ones: those Peios ships, read once, since only a package
/// upgrade changes them.
pub struct Known {
    shipped: HashMap<String, Details>,
}

impl Known {
    pub fn read() -> Known {
        let shipped = std::fs::read_to_string(SHARE_BUNDLE)
            .map(|text| {
                trust_client::from_pem(&text)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|der| {
                        Some((
                            trust_client::fingerprint(&der).ok()?,
                            trust_client::inspect(&der).ok()?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Known { shipped }
    }

    fn name(&self, fingerprint: &str, added: &HashMap<String, Details>) -> Option<Details> {
        self.shipped
            .get(fingerprint)
            .or_else(|| added.get(fingerprint))
            .cloned()
    }
}

/// The additions in the registry, by fingerprint, whether in force or not.
fn added() -> HashMap<String, Details> {
    let mut out = HashMap::new();
    let path = format!("{CERTIFICATES_KEY}\\{ADD_KEY}");
    let Ok(add) = Key::open(None, &path, KeyAccess::READ, OpenFlags::empty()) else {
        return out;
    };
    for name in add.subkeys(None).flatten() {
        let Ok(entry) = Key::open(
            Some(&add),
            &String::from_utf8_lossy(&name.name),
            KeyAccess::QUERY_VALUE,
            OpenFlags::empty(),
        ) else {
            continue;
        };
        let Ok(value) = entry.query_value(CERTIFICATE_VALUE.as_bytes(), None) else {
            continue;
        };
        if let (Ok(fingerprint), Ok(details)) = (
            trust_client::fingerprint(&value.data),
            trust_client::inspect(&value.data),
        ) {
            out.insert(fingerprint, details);
        }
    }
    out
}

/// How long the window waits for trustd to answer, as `trust` waits.
const ANSWER: Duration = Duration::from_secs(10);

/// trustd, at its socket.
pub fn trustd() -> Result<Client, trust_client::Error> {
    Client::system(ANSWER)
}

/// What trustd says is in force, each certificate described.
fn roots() -> Result<Vec<Cert>, String> {
    let mut certs: Vec<Cert> = trustd()
        .and_then(|trustd| trustd.roots(true, None))
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|mut root| {
            let details = trust_client::inspect(&root.der).unwrap_or_else(|_| Details {
                subject: root.subject.clone(),
                ..Details::default()
            });
            // The window keeps the certificate for Export; the rest of what
            // it shows is in the details.
            root.der.shrink_to_fit();
            Cert { root, details }
        })
        .collect();
    certs.sort_by_cached_key(|c| c.name().to_lowercase());
    Ok(certs)
}

/// Reads everything. `roots` is whether to ask trustd for the certificates
/// as well, which is the only part that is large; otherwise `before`'s are
/// kept.
pub fn read(known: &Known, before: Option<&Store>) -> Store {
    let roots = match before {
        Some(store) if store.roots.is_ok() => store.roots.clone(),
        _ => roots(),
    };
    read_with(known, roots)
}

/// Reads everything, the certificates included.
pub fn read_all(known: &Known) -> Store {
    read_with(known, roots())
}

fn read_with(known: &Known, roots: Result<Vec<Cert>, String>) -> Store {
    let added = added();
    let distrusted = trust_client::distrusted()
        .map_err(|e| e.to_string())
        .map(|entries| {
            let mut refused: Vec<Refused> = entries
                .into_iter()
                .map(|d| Refused {
                    known: known.name(&d.fingerprint, &added),
                    fingerprint: d.fingerprint,
                    reason: d.reason,
                })
                .collect();
            refused.sort_by_cached_key(|r| {
                (
                    r.known.is_none(),
                    r.known.as_ref().map(|d| d.name().to_lowercase()),
                )
            });
            refused
        });
    Store {
        roots,
        status: trustd().and_then(|trustd| trustd.status()).ok(),
        distrusted,
        may: trust_client::may_change().map_err(|_| LOOK_ONLY.to_string()),
        may_compat: trust_client::may_set_compat().map_err(|_| LOOK_ONLY.to_string()),
        may_reload: trust_client::may_reload().unwrap_or(false),
        keys: crate::keys::read(),
    }
}

/// How long the window goes without hearing from trustd before it looks
/// for itself: a degraded store sends nothing to subscribers.
const QUIET: Duration = Duration::from_secs(10);

/// Follows trustd for as long as the window is there: every set it sends a
/// subscriber is a change, after which everything is read again. Between
/// changes, what is cheap to read is read every little while.
pub fn follow(window: Weak<Surface<Policy>>, known: Arc<Known>) {
    loop {
        // A subscriber is sent the set as it is at once; that is not news.
        let subscribed = trustd()
            .and_then(|trustd| trustd.subscribe(false))
            .and_then(|mut subscription| subscription.next(ANSWER).map(|_| subscription));
        let Ok(mut subscription) = subscribed else {
            std::thread::sleep(QUIET);
            if !tell(&window, read(&known, None)) {
                return;
            }
            continue;
        };
        loop {
            match subscription.next(QUIET) {
                Ok(_) => {
                    if !tell(&window, read_all(&known)) {
                        return;
                    }
                }
                // Quiet for a while: what is cheap is read. A wait that
                // runs out keeps what had arrived, so the subscription
                // goes on.
                Err(e)
                    if std::io::Error::from_raw_os_error(e.code).kind() == ErrorKind::TimedOut =>
                {
                    let before = window
                        .upgrade()
                        .map(|shown| shown.look(|policy, _, _| policy.store().clone()));
                    let Some(before) = before else { return };
                    if !tell(&window, read(&known, Some(&before))) {
                        return;
                    }
                }
                // trustd went away; subscribe again when it is back.
                Err(_) => {
                    std::thread::sleep(QUIET);
                    break;
                }
            }
        }
    }
}

/// Gives the window what was read, if it differs. False once the window
/// has gone.
fn tell(window: &Weak<Surface<Policy>>, read: Store) -> bool {
    let Some(shown) = window.upgrade() else {
        return false;
    };
    if shown.look(|policy, _, _| *policy.store() != read) {
        shown.update(|policy, fields| policy.heard(read, fields));
    }
    true
}
