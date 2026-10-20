---
title: Managing Features
type: how-to
description: Configure installed feature definitions without confusing them with packages.
---

Feature Manager controls feature definitions supplied by installed
packages. A feature performs configuration beyond copying package files,
such as setting up services. Installing its package and enabling its
feature are different operations.

## Before you start

You need Feature Manager and permission to run the requested operation
through `feat`. Read what the feature changes before applying it.

## Choose the feature state

1. Open **Feature Manager** and inspect the feature's description and
   state.
2. Choose **Turn On** to install and enable its setup, **Install Only** to
   set it up without enabling it, or **Turn Off** to disable it while
   retaining its installed setup.
3. To remove the feature's setup, choose **Remove** and review the
   confirmation. Its package remains installed.
4. Follow the card's progress and read the outcome or failed script output.

## Check the result

Confirm the final feature state and verify the affected service or
setting. **Interrupted** identifies a script that did not finish;
**Try Again** reruns it. Review the failure before retrying.
**Definition Missing** means installed feature state survives after its
definition package was removed.

An approved operation continues until completion when the window is asked
to close. The window stays open with an explanation and closes afterward.
Feature Manager delegates authority and state changes to `feat`; it does
not grant privileges. See `gxwi-feature-manager(1)`.

## Missing definitions and limited access

If a definition is missing, restore its package before asking `feat` to run
the feature's cleanup. Removing a package does not undo the configuration its
feature created. Keep the package available until the feature is removed.

The read-only view can show feature states without offering change buttons.
Permission to change state does not supply every privilege a feature script
needs. Read the failed script's output and investigate its access requirements
before retrying; the app does not elevate it automatically.
