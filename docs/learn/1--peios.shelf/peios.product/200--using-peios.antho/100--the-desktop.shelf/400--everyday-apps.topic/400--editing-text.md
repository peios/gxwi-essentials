---
title: Editing Text Files
type: how-to
description: Open, save, and resolve external changes to a small text file.
---

Text Editor edits one file per window. Unsaved text belongs to the
application window, so reconnecting a browser does not discard it. Closing
the application window asks about unsaved changes.

## Before you start

You need Text Editor in your GXWI session and permission to read the file.
The editor accepts UTF-8 text without NUL bytes, up to 349,525 bytes. Use a
tool suited to larger or binary files when a file exceeds those limits.

## Edit and save

1. Open a text file from File Explorer, or start **Text Editor** for a new
   document.
2. Make the intended change. Press **Ctrl+S** to save. A new document asks
   for a name when first saved.
3. To save under another name, press **Ctrl+Shift+S** and choose an unused
   name. This operation does not overwrite an existing file.

## Check the result

Read the result shown by the editor and confirm that the document has no
unsaved changes. Saving an existing file writes it in place,
preserving its identity rather than replacing it with another file.
Uniform CRLF line endings are preserved; mixed endings are written as LF.

## Changes made elsewhere

The editor checks the file every two seconds. If another program changes
it while your buffer is unchanged, the editor reloads it. If both versions
changed, it asks which to keep. Read both the conflict and the file's
purpose before discarding either version.

Saving in place is not an atomic replacement transaction. Keep a backup
when editing a file whose previous contents need to remain recoverable.
See `gxwi-text-editor(1)` for the supported input and shortcuts.
