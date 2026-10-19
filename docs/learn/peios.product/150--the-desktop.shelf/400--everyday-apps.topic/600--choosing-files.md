---
title: The File Dialog
type: concept
description: Understand what an application does when it asks for a file.
---

The shared file dialog lets an application ask you for a file to open or a
name to save. The dialog chooses a path; the calling application opens or
writes that path. A successful selection does not grant the caller new
filesystem rights.

The dialog shows folders first and filters files by the kind the caller
requested. **Up**, **Home**, and the path field change the location.
**Hidden files** includes names beginning with a dot. In save mode, the
selected kind can supply a missing extension, and an occupied name asks
for confirmation.

The dialog can also create a folder, rename an entry, or delete it.
Deletion is real: a directory goes with its contents. The dialog runs as
you and disables operations it cannot perform.

Canceling, pressing **Esc**, or closing the dialog reports cancellation to
the caller. The dialog also ends when its caller closes the input
channel. It is not meant to be started as a standalone launcher task.

Developers use the file-choosing protocol or the companion Rust client.
The protocol's shared contract belongs to the Peios userspace
specifications; this suite owns the dialog implementation. See
`gxwi-file-dialog(1)` for transport and lifecycle details.

The selection protocol returns a UTF-8 path. A path that cannot be
represented as UTF-8 is refused explicitly and leaves the dialog open.
Local operations still identify the original entry independently of its
displayed label. Rename refuses an occupied destination, including one
created after the dialog's initial check.
