---
title: Managing Services in Services Manager
type: how-to
description: Inspect a service and control its runtime or edit its definition.
---

Services Manager distinguishes a service's running state from the
persistent definition peinit reads. Starting or stopping a service changes
its runtime; editing the definition changes how it is configured.

## Before you start

You need Services Manager in a GXWI session. Runtime control uses peinit's
service rights. Editing a definition uses registry rights, which are a
separate permission check.

## Control a service

1. Open **Services Manager** and use **Find a service** to narrow the list.
2. Select the service and read its state, reason, process, account, recent
   report, and timers. Timer times are shown in local time.
3. Choose the available runtime action, such as **Start**, **Stop**,
   **Restart**, or **Reload**, after checking its effect.
4. Read the command's outcome. Select **Logs** to open the service's
   recorded output in Event Viewer when more context is needed.

## Check the result

The state refreshes every two seconds. **Refresh** or **F5** reads it at
once. Confirm the intended state and inspect any failure reason. A button
being available does not guarantee that the service will start
successfully.

## Edit a definition

Open **Edit definition** for an existing service or **New service** for a
new one. Review the grouped fields and save with **Save** or **Ctrl+S**.
The editor validates the fields and writes changed values in one registry
transaction. Check peinit's resulting state rather than assuming that a
saved definition restarted an existing process.

The permissions for controlling a service and changing its definition are
separate dialogs. **Default permissions** concerns services without their
own service descriptor. A registry definition you can read can remain
visible even when peinit refuses access to the service's runtime state.
See `gxwi-services-manager(1)` and `svctl(8)`.
