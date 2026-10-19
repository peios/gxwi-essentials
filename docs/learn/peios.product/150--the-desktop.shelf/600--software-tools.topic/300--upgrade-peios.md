---
title: Upgrading Peios from the Desktop
type: how-to
description: Review a release upgrade and choose when its registry seeds apply.
---

Upgrade Peios changes the machine's Peios release through `upgrade-peios`.
Package Manager handles other software updates. This window uses the same
release plan and permissions as the upgrade command.

## Before you start

You need permission to upgrade, trusted repository metadata, and a recovery
plan appropriate to the release. Read the shared release-upgrade guidance
before applying a machine-wide change.

## Review and apply the upgrade

1. Open **Upgrade Peios**. Check the current release and edition package.
   Use **Check Now** to refresh already trusted repositories and request a
   plan without applying it.
2. Read every proposed change, download size, and installed-size effect.
3. Choose whether **Apply Its Settings at the Next Restart** should defer
   the release's registry seeds. Review the upgrade confirmation and any
   separate package approvals.
4. Start the upgrade and follow it to completion. If the plan changed
   before execution, review the new plan before accepting it.

## Check the result

Read the final outcome and installed release state. The application does
not restart the machine. If settings are deferred, verify that pending
state and apply them at the planned restart. **Apply Now** requests their
immediate application instead.

Once approved, the transaction continues to its result even if the window
is asked to close. **Continue Anyway** after stale-repository information
is an explicit choice to use the available metadata, not a successful
refresh. See `gxwi-upgrade-peios(1)` and the upgrade command's documentation.


## Recovering from a refusal or incomplete result

An untrusted repository needs confirmation in Package Manager; checking
for a release does not grant that trust. When another package change holds
the backend lock, wait for it to finish before trying again.

A settings-application failure does not undo an installed release. Inspect
the pending seeds and the reported package result separately. **Apply
Now** asks the release updater to apply waiting seeds; a restart also
provides the configured application point.

The driven upgrade protocol reports an explicit cancellation separately
from completion. A canceled result stops before release seeds are staged
or applied. Already completed backend effects are not a general rollback
transaction.

An installed package version does not establish which kernel or services
are running. Follow the shared release-upgrade activation checks before
relying on the update. The desktop provides a result display, not proof
that all changed software has restarted.
