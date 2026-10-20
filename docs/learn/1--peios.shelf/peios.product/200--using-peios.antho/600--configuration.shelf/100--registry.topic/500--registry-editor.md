---
title: Registry Editor
type: concept
description: Understand typed values, layers, permissions, and recovery operations.
---

Registry Editor displays keys, values, and their registry documentation.
A key is a container; a value is named data with a type. The editor reads
as your account and distinguishes unavailable access from an empty key.

The tree starts at **Machine**, **Users**, and **CurrentUser**. A direct
path can reach a readable child even when you cannot list its parent.
The detail pane shows a value's data, type, size, and winning layer, or
information about the selected key.

## Stored data and documented defaults

The registry manual supplies descriptions, expected types, defaults, and
application timing. Documented values absent from the key are shown as
**Not set**. These are explanatory entries, not stored data or exported
values. A type mismatch or malformed value is shown instead of being
silently repaired.

A saved registry value can still be rejected by its consuming component.
Follow its manual and inspect its status after a change.

## Layers

**Writes go to** selects the destination layer. A value's displayed winner
can come from another layer. Writing a lower layer does not displace an
entry with higher precedence. Deleting a value removes the selected
layer's entry, so another entry can become visible instead of the value
becoming absent.

Layer management changes whole sets of entries. Positive precedence needs
the operating-system privilege `SeTcbPrivilege`; being an administrator is
not sufficient by itself. Key owners and permissions are not layered:
disabling a layer does not undo a descriptor change.

## Export and backup

**Export** creates a reviewable registry document of keys and values.
**Back up** includes security descriptors and tagged layer entries.
They are not interchangeable recovery formats. A backup's layer manifest
does not recreate missing layer definitions on its own.

**Import** writes the selected document into the chosen layer in a
transaction. **Restore** replaces the target's contents and subtree,
including its descriptor, rather than merging into the current contents.
Backup and restore have their own privilege requirements.

For a controlled edit, follow
[Changing Registry Values](~peios/changing-registry-values). The command
reference is `gxwi-registry-editor(1)`; the registry's shared semantics
remain owned by its own documentation.
