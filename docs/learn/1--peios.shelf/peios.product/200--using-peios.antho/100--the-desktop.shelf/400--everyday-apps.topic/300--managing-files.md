---
title: Managing Files
type: how-to
description: Copy, move, rename, or remove an entry in File Explorer.
---

Use File Explorer to organize files within the access your account has.
Copying keeps the original; cutting and pasting moves it. Deleting removes
it without a trash folder.

## Before you start

You need File Explorer in a running GXWI desktop and access to the source
and destination directories. Keep a separate copy before deleting data
you might need again.

## Copy or move an entry

1. Open **File Explorer** from the launcher and navigate to the source
   directory.
2. Open the entry's context menu and choose **Copy** or **Cut**.
3. Open the destination directory in this or another File Explorer
   window. Open the directory's context menu and choose **Paste**.
4. Wait for the operation's result before starting another operation on
   the same entry.

## Check the result

Inspect the destination entry and open it when appropriate. A copy leaves
the source intact; a successful move removes it from the old directory.
If a cut name is already occupied, choose a different destination or rename
an entry after reviewing what is there.

## Rename or delete

Select **Rename** from an entry's context menu and enter an unused name.
File Explorer refuses an occupied destination instead of replacing it.

For deletion, review the selected entry, choose **Delete**, and read the
confirmation. Confirm only the intended entry and subtree. Check the
listing afterward; a refusal means the operation did not complete as
requested and needs review, not repeated confirmation without inspection.

Use the directory context menu to create a new folder or file. Its name
must identify a new entry. The manual `gxwi-file-explorer(1)` describes
launch arguments and listing limits.
