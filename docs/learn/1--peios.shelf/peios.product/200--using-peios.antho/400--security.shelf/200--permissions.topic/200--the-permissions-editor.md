---
title: The Permissions Editor
type: concept
description: Understand descriptor editing and the authority retained by its caller.
---

The permissions editor is a shared dialog for security descriptors.
A caller identifies the object, supplies the readable descriptor parts and
named rights, and applies changes returned by the dialog. The dialog does
not gain authority over the object or write its descriptor directly.

## Views of one descriptor

**Access & Auditing** presents identities, grants, denies, conditions,
specific rights, and audit rules. Other views cover resource claims,
central access policies, and effective access. Owner, group, integrity,
and process trust are available through the owner controls.

**Advanced Mode** shows the ordered access-list entries, control flags,
and text representation. Entries the simpler views cannot edit remain
preserved. An unchanged entry is returned with its original bytes.
Unsupported text-form entries are represented by retention comments;
removing such a comment removes that entry.

The caller's declared capabilities restrict what can be changed. An
otherwise valid owner also has to satisfy the kernel's ownership rules.
A read-only dialog reports why it cannot apply changes.

## Inheritance and existing objects

Changing a container's inheritable rules governs new children. When the
caller supports propagation, the dialog asks whether to update existing
children too. The caller performs that operation, reports progress, and
handles a stop request. Read the final report of failures; partial
propagation is not a completed update of every child.

## Claim suggestions

The dialog suggests claims declared in the machine's Security Descriptor
Builder catalog, then claims and values recorded by permitted callers
when rules were applied. Suggestions help spelling; they do not create a
claim on the caller's token or make a condition true.

See `gxwi-sd-editor(1)` and the registry manual for
`Machine\Common\SecurityDescriptorBuilder`. Shared descriptor semantics
and the dialog protocol remain owned by the Peios specifications.
