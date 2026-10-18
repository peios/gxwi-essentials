//! Pushing what a container passes down into what is already inside it
//! (PCDS §5.6, Re-propagation), for a program that holds a tree of things
//! with descriptors: a folder's files, a registry key's subkeys.
//!
//! The program says how to get about its tree ([`Tree`]); [`walk`] does
//! the rest, the same for every kind of tree: parents first, each item
//! re-propagated from its parent as just rewritten through libpeios'
//! `reinherit_with`, a list an item protects left as it is, a failure
//! noted and the walk carried on past it.

use std::sync::atomic::{AtomicBool, Ordering};

use peios::file::SecInfo;
use peios::security::{Control, GenericMapping, SdView, reinherit_with};

use crate::{Failure, Generic, Part, Walked};

/// A tree of things with descriptors, as the program holding it gets
/// about it.
pub trait Tree {
    type Node: Clone;

    /// What is directly inside `node`.
    fn children(&mut self, node: &Self::Node) -> Result<Vec<Self::Node>, String>;

    /// What `node` is called where the person knows it, for progress and
    /// failures: `/srv/finance/q3.xlsx`.
    fn name(&self, node: &Self::Node) -> String;

    /// Whether `node` is a container, which things inherit from.
    fn container(&self, node: &Self::Node) -> bool;

    /// The parts of `node`'s descriptor `info` asks for.
    fn read(&mut self, node: &Self::Node, info: SecInfo) -> Result<Vec<u8>, String>;

    /// Applies the parts of `sd` that `info` names to `node`.
    fn write(&mut self, node: &Self::Node, sd: &[u8], info: SecInfo) -> Result<(), String>;
}

/// Which lists `parts` re-propagates: as they are read and written, and as
/// libpeios is asked to work them out. The integrity label alone is read
/// and written as the label, and worked out as a SACL holding only it.
fn lists(parts: &[Part]) -> (SecInfo, SecInfo) {
    let mut io = SecInfo::empty();
    let mut work = SecInfo::empty();
    if parts.contains(&Part::Dacl) {
        io |= SecInfo::DACL;
        work |= SecInfo::DACL;
    }
    if parts.contains(&Part::Sacl) {
        io |= SecInfo::SACL;
        work |= SecInfo::SACL;
    } else if parts.contains(&Part::Label) {
        io |= SecInfo::LABEL;
        work |= SecInfo::SACL;
    }
    (io, work)
}

/// Pushes what `root` passes down, in the lists `parts` names, into
/// everything inside it, `generic` being the mapping of the things in the
/// tree. Says how far it has got as each item is done, and stops after the
/// item in hand once `stop` is set.
pub fn walk<T: Tree>(
    tree: &mut T,
    root: &T::Node,
    parts: &[Part],
    generic: Generic,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, &str),
) -> Walked {
    let mut out = Walked::default();
    let (io, work) = lists(parts);
    if io.is_empty() {
        return out;
    }
    let mapping = GenericMapping::new(generic.read, generic.write, generic.execute, generic.all);
    let root_sd = match tree.read(root, io | SecInfo::OWNER | SecInfo::GROUP) {
        Ok(sd) => sd,
        Err(why) => {
            out.failed.push(Failure {
                name: tree.name(root),
                why,
            });
            return out;
        }
    };
    // Containers whose insides are still to be done, with their
    // descriptors as they are now.
    let mut todo = vec![(root.clone(), root_sd)];
    while let Some((node, sd)) = todo.pop() {
        let inside = match tree.children(&node) {
            Ok(inside) => inside,
            Err(why) => {
                out.failed.push(Failure {
                    name: tree.name(&node),
                    why: format!("what is inside could not be listed: {why}"),
                });
                continue;
            }
        };
        for item in inside {
            if stop.load(Ordering::Relaxed) {
                out.stopped = true;
                return out;
            }
            let name = tree.name(&item);
            match one(tree, &item, &sd, io, work, &mapping) {
                Ok(now) => {
                    out.done += 1;
                    progress(out.done, &name);
                    if tree.container(&item) {
                        todo.push((item, now));
                    }
                }
                // What is inside one that could not be done is left: it
                // would be worked out from what that one passes down now.
                Err(why) => out.failed.push(Failure { name, why }),
            }
        }
    }
    out
}

/// Re-propagates one item from its parent's descriptor `parent`, and gives
/// its descriptor as it is afterwards.
fn one<T: Tree>(
    tree: &mut T,
    item: &T::Node,
    parent: &[u8],
    io: SecInfo,
    work: SecInfo,
    mapping: &GenericMapping,
) -> Result<Vec<u8>, String> {
    // The owner and group too, which CREATOR OWNER and CREATOR GROUP
    // resolve to.
    let sd = tree.read(item, io | SecInfo::OWNER | SecInfo::GROUP)?;
    let control = SdView::parse(&sd)
        .map_err(|e| format!("its descriptor could not be read ({e})"))?
        .control();
    let (mut io, mut work) = (io, work);
    if control.contains(Control::DACL_PROTECTED) {
        io.remove(SecInfo::DACL);
        work.remove(SecInfo::DACL);
    }
    if control.contains(Control::SACL_PROTECTED) {
        io.remove(SecInfo::SACL | SecInfo::LABEL);
        work.remove(SecInfo::SACL);
    }
    if io.is_empty() {
        return Ok(sd);
    }
    let new = reinherit_with(parent, &sd, tree.container(item), Some(mapping), work)
        .map_err(|e| format!("what it inherits could not be worked out ({e})"))?;
    tree.write(item, new.as_bytes(), io)?;
    Ok(new.as_bytes().to_vec())
}

#[cfg(test)]
#[path = "propagate_tests.rs"]
mod tests;
