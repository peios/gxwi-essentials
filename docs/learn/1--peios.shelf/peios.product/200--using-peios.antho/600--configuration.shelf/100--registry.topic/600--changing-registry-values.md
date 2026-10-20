---
title: Changing Values in Registry Editor
type: how-to
description: Choose the write layer and verify both storage and application.
---

A registry edit changes stored configuration. To establish that the change
worked, verify both the resulting entry and the component that reads it.

## Before you start

You need Registry Editor and access to the exact key and destination
layer. Record the existing value, type, layer, and relevant permissions.
Choose an export or backup suited to the recovery you need.

## Edit one value

1. Open **Registry Editor** and navigate to the key, or enter its path and
   select **Go**.
2. Select the value and read its registry manual, including valid values
   and when a change takes effect.
3. Check **Writes go to**. Confirm the selected destination even when a
   different layer supplies the displayed value.
4. Open **Edit**, enter the intended data, and select **Save**. For a
   documented but absent value, use **Set**.
5. If the editor reports a concurrent change, refresh and inspect the new
   state before submitting another edit.

## Check the result

Use **Refresh** or **F5**. Inspect the value, type, and winning layer.
Then inspect the consuming component at the time its manual requires.
A conditional write protects the destination layer's entry, not every
possible change to a winner in another layer.

## Deleting and recovering

Deleting a key removes its subtree in one transaction or fails. Large
subtrees can exceed transaction or open-handle limits. Agree the intended
smaller scope before splitting a failed deletion, because several
transactions are not one atomic operation.

Deleting a value can reveal a surviving entry from another layer. A saved
change or deletion does not by itself select the consumer's compiled
default.

Read an import or restore target before confirming it. After a timeout or
source failure, inspect current state before retrying: an error alone does
not prove that a transaction was rolled back. Changing permissions affects
future opens; existing handles retain rights already granted.
