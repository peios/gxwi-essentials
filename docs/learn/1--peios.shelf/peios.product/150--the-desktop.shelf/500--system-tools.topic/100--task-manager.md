---
title: Inspecting Tasks and Sessions
type: how-to
description: Find the right process and choose the scope of a stop operation.
---

Task Manager shows processes, resource use, jobs, and signed-in sessions.
Use it to identify what is running before deciding whether to end one
process, stop its service or job, or sign out a whole session.

## Before you start

You need Task Manager in a GXWI desktop. Access controls can hide process
details or sessions and can prevent a stop operation. Missing details are
not proof that the process has no owner or command.

## Inspect and act

1. Open **Task Manager** and select **Processes**. Use **Find** or sort by
   **CPU** or **Memory** to locate the process.
2. Select it and inspect its command, owner, parent, service or job, and
   session. **Group by** organizes the list by service or person.
3. Choose the smallest intended scope. **End process** targets one
   process; **Stop service** and **Stop job** ask peinit to stop their
   managed workload.
4. Review the confirmation. After an end request, a process still running
   after five seconds can offer **End it now**, which does not allow
   orderly completion.

## Check the result

Select **Refresh** or press **F5** and inspect the process and its managed
state. Ending a service's main process can trigger its restart policy.
Use **Stop service** when the intended result is a stopped service.
Task Manager retains a process handle for termination so a reused process
ID does not redirect that request to a different process.

## Sessions and visibility

**Signed in** lists visible logon sessions and their processes.
**Sign out** ends the selected session through authd; review carefully
when it is your own session. A refusal or a process that cannot be ended
can leave the session open. A service session is managed by its service.

The full kernel session list requires the corresponding access. Otherwise,
Task Manager lists the sessions it can infer from visible processes.
Protected processes can remain listed while their details and control are
unavailable, including to an administrator whose process lacks the needed
protection level. See `gxwi-task-manager(1)`.
