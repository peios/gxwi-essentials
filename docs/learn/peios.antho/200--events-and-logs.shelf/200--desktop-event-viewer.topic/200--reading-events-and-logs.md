---
title: Reading Events and Logs
type: how-to
description: Narrow a record list while keeping access and retention limits visible.
---

Use Event Viewer to find a recorded event or a service's logs. Narrow the
query enough to inspect the relevant records, then retain the context
needed to interpret them.

## Before you start

You need Event Viewer, a running eventd query service, and access to the
records. Know the approximate time, origin, or event type involved.

## Find a record

1. Open **Event Viewer** and select **Events** or **Logs**. Services
   Manager's **Logs** action can open the selected service's origin.
2. Set the relevant type or source for events, or origin and text filter
   for logs. Use the errors-only filter when that matches the question.
3. Select **Pause** if new rows make inspection difficult. Select a row
   to inspect its full visible fields.
4. Use **Show older** to extend the displayed history and **Resume** when
   you want live updates again.

## Check the result

Read the record's time, type or origin, and fields, together with any
visibility note. A list is bounded to 1,000 rows, so use a narrower query
or `evctl` when your investigation needs a different range or query shape.

If eventd refuses the connection, address that refusal rather than treating
the empty display as a result. If records remain absent, check the filter,
retention, and read policy before concluding that nothing was recorded.
