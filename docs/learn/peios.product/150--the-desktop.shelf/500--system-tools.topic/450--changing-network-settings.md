---
title: Changing Network Settings
type: how-to
description: Review a network change and verify it before keeping it.
---

Use Network Manager to change the intended profile, selection rule, or
firewall rule, then verify its live effect. Some changes can disconnect the
desktop through which you are working.

## Before you start

You need permission to change the relevant settings. Record their previous
values and keep another recovery path for changes that can affect remote
access. Not every change has automatic undo.

## Apply a controlled change

1. Open **Network Manager** and inspect the affected interface in
   **Overview**. Record the profile, rule, network, and addresses it uses.
2. Open the relevant profile, profile rule, or firewall rule. Inspect the
   preview and any affected interfaces or desktop connections.
3. Save the intended change. If the application warns that it would
   disconnect an active desktop, read the affected connections before
   deciding whether to continue.
4. When **Keep this change?** appears, test the intended connection before
   selecting **Keep**. Select **Undo** to revert, or allow the 30-second
   server-side timer to expire.

## Check the result

Return to **Overview** and check the actual interface state. For a name
change, perform the intended lookup; for a firewall change, test the actual
connection. Read the accepted policy generation and any refusal. Example
firewall activity is not live confirmation.

## Which changes have undo

The keep timer covers firewall and profile-rule changes, exposure changes,
and relevant changes to profiles or known networks carrying desktop
connections. A change accepted after a disconnect warning also receives
the timer. Starting another guarded change keeps the preceding one.

DNS settings, static names, port reservations, audit levels, and new
profiles can apply immediately without that bar. Restore their recorded
previous values explicitly if needed. The server runs the guarded timer,
so losing the browser connection does not prevent its expiry.

See `gxwi-network-manager(1)`, `net(1)`, and `resolv(1)` for command
interfaces and backend behavior.
