---
title: File Explorer
type: concept
description: Understand file browsing, opening, clipboard operations, and deletion.
---

File Explorer shows a directory and opens its contents with the installed
applications. A window owns its current directory, selection, and sort
order. Devices showing the same session window see that shared state;
open another window to browse a different location independently.

## Opening an entry

Opening a folder navigates into it. Opening an executable starts it.
Other files use an application that declares support for their file type:
your default takes precedence over the machine default, then the available
candidates are ordered by title. The entry menu lets you choose another
candidate.

The listing identifies file types from their names without reading every
file. When an unknown type is opened, File Explorer can check whether its
contents are text. This probe reads at most 4 KiB from a regular file; it
does not wait for data from a pipe or device. A directory's menu offers
applications that open
directories, including Terminal when it is installed.

A directory listing displays at most 2,000 entries and reports when the
limit is reached. An incomplete listing is not evidence that an omitted
file is absent.

## Copying and moving

Cut and copy select an entry for a later paste. That selection is shared
between File Explorer windows through the session runtime directory.
Without that directory, the selection belongs to one window.

Pasting a copy under an occupied name chooses a copy name. Pasting a cut
entry does not overwrite an existing name; the entry remains selected for
a later move. Copying a directory includes its contents and runs while the
window continues to respond.

## Removal and permissions

Delete asks for confirmation and removes the entry. A directory is removed
with its contents. There is no trash or undo store, so preserve anything
you need before confirming.

**Permissions** opens the shared descriptor editor. File Explorer retains
a handle to the selected object so applying a descriptor targets that
object even if its name changes. The dialog does not supply extra rights.
See [The Permissions Editor](~peios/the-permissions-editor) and
`gxwi-file-explorer(1)`.

## Names that are not UTF-8

A filename that cannot be displayed as UTF-8 uses replacement characters
in the listing. Local navigation, rename, and deletion retain the original
filename bytes, so identical-looking labels do not identify the same entry.
The shared clipboard accepts only UTF-8 paths and reports other paths as
unsupported rather than silently changing their names.
