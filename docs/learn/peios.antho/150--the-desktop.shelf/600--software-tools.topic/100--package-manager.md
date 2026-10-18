---
title: Managing Packages
type: how-to
description: Review and apply software transactions through Package Manager.
---

Package Manager presents peipkg's package queries and transactions. It runs
as your account and uses the same plans, permission checks, history, and
audit records as the command-line package manager.

## Before you start

You need the package database access required for the task, a running GXWI
session, and the relevant repository or package file. Reading package
records and changing installed software are separate permissions.

## Review and apply a transaction

1. Open **Package Manager**. Use **Installed**, **Available**, or
   **Updates** to select the package and action.
2. Read the plan: packages to install, update, downgrade, or remove, and
   the expected download and installed sizes.
3. Review any separate approval for an elevated action, such as replacing
   unowned files or accepting a downgrade. Approve only the action you
   intend.
4. Confirm the plan and follow progress to the final outcome. Answer a
   modified-configuration-file question only after reviewing that file.

## Check the result

Inspect the final outcome, warnings, and **History**, then verify the
installed package version and intended application behavior. **Check
Files** compares installed files with package records. A transaction that
was interrupted can offer **Resolve**; the most recent eligible
transaction can offer **Undo**.

After plan approval, closing the window does not cancel the transaction.
The application waits for it to finish. Do not assume a lost browser
connection rolled back a change.

## Repositories, roles, and roots

**Repositories** lists sources and offers refresh, add, trust, and removal
operations. Trust requires your explicit review of the signing key;
update checks refresh repositories already trusted. Stale metadata and
publisher-controlled update refusals have separate, explicit overrides.

**Roles** shows claims and their owning packages. **Installation Roots**
shows named roots; creating them is a `peipkg root` task. **Install from
File** uses the shared file dialog for a `.peipkg` file. See
`gxwi-package-manager(1)` and `peipkg(1)`.
