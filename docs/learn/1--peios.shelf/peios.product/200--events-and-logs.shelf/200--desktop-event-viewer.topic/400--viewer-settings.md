---
title: Changing Event Viewer Policy Settings
type: how-to
description: Use eventd metadata to edit a setting or a read-policy pattern.
---

Event Viewer's Settings tab exposes eventd's configuration and read policy.
Configuration controls the service; policy controls which names and fields
accounts can read. Your personal dashboards are separate preferences.

## Before you start

You need write access for the selected configuration or policy entry.
Record its current value or descriptor and read eventd's registry manual.
Socket and storage paths are displayed but are not editable here.

## Change and verify

1. Open **Settings** and select the relevant configuration group or
   read-policy pattern.
2. Read the expected type, range, default, and application timing. Enter
   the value and save, or use the permissions editor for a pattern.
3. Inspect the result. A value outside the accepted range is refused.
4. Verify the service's active behavior at the stated time. For access
   policy, query as the affected account rather than only as an
   administrator.

## Pattern scope

A pattern can cover a name and its descendants. The most specific matching
pattern wins. Removing a specific pattern exposes the broader policy; it
is not necessarily a denial. The fallback `*` patterns cannot be removed
through this interface.

A new pattern starts from the permissions that already apply to its name
and changes only when the descriptor is applied. Confirm the intended
scope before adding a broad grant or deny. Use eventd's own documentation
for the precise access model and configuration semantics.
