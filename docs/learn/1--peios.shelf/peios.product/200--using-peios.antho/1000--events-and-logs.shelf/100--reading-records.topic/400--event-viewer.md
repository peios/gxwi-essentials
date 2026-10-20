---
title: Event Viewer
type: concept
description: Read recorded events, logs, metrics, and their visibility limits.
---

Event Viewer reads this machine's records from eventd. It separates
**Events**, **Metrics**, **Logs**, and **Settings**, and performs queries as
your account. The backend filters records according to their read policy.

Events and logs are displayed newest first. A list starts with the newest
200 records; **Show older** adds another 200, and a window holds at most
1,000. **Pause** keeps the list still and **Resume** continues updates.
Selecting a row shows its available fields.

Event filters narrow by type and source. Log filters narrow by origin,
text, or errors. Metrics use named dashboards of charts rather than the
row list. Dashboard preferences are personal and stored under
`CurrentUser\Software\gxwi-event-viewer`.

## Interpreting missing records

A missing event is not proof that it did not occur. Retention, query
filters, time range, and access policy all affect the result. Event Viewer
tries to read the policy and explains known hidden names. When that policy
cannot be read, it reports that some data could be hidden rather than
claiming complete visibility.

The Settings tab edits eventd configuration and read policy only where
access permits it. It uses eventd's registry manual for meaning, defaults,
ranges, and application timing. The viewer's dashboard schema belongs to
its own registry manual.

See [Reading Events and Logs](~peios/reading-events-and-logs) and
`gxwi-event-viewer(1)`.
