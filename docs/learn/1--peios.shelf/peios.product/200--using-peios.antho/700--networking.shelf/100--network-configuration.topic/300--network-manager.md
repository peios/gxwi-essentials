---
title: Network Manager
type: concept
description: Distinguish live network settings from example firewall displays.
---

Network Manager combines interface configuration, name resolution, and
firewall policy in one desktop application. It reads netd, resolvd, kernel
state, and registry settings as your account. Each operation remains
subject to the backend's access checks.

## Finding the right view

The application groups its views by purpose:

| Group | Views and purpose |
|---|---|
| Network | Overview, known networks, profiles, profile rules, and DNS |
| Firewall | Exposure, rule editing, connection tests, and activity |
| System | Port reservations, audit level, and advanced daemon settings |

**Overview** maps an interface to its selected profile and network, with
addresses, routes, traffic, and lease information. A **Connected** summary
means there is a route; it does not prove that a particular service or the
Internet is reachable. Wireless interfaces are shown as unmanaged by this
implementation.

**Networks** records identities and trust levels. **Profiles** define
configuration and **Profile rules** choose where it applies. **DNS**
queries resolvd and manages fallback servers, search domains, static names,
and its cache. Port-reservation permission to bind a socket is separate
from firewall permission for traffic to reach it.

## Example firewall data

The listener, connection, counter, and recent-audit displays under the
firewall interface use constructed examples. Affected panels say
**Example data**, and the status line says **Example**. They use real
rules and some real machine context, but are not a live inventory of
traffic or listening services.

The saved rules and network settings are real. A connection test predicts
a policy result; verify a policy change with the actual connection or
service you intended to affect. Do not use an example activity panel as
evidence that a connection happened or was blocked.

## Permissions and refresh

Settings are checked for write access by registry key. Runtime **Renew
lease** and **Reapply** use netd's control right. Read-only panels explain
why a change is unavailable. Exposure and Activity also restrict access.

Live machine data refreshes every two seconds; registry changes trigger
settings refreshes. A daemon refusal is shown with the previous accepted
configuration or generation still in force. See
[Changing Network Settings](~peios/changing-network-settings) before making
a change through a remote desktop.
