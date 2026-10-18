---
title: Managing Local Accounts
type: how-to
description: Inspect an identity before changing membership or sign-in settings.
---

Use Principals Manager for a local user's membership, credentials, claims,
and permitted sign-in methods. Check the identity and the effect on future
sign-ins before making a change.

## Before you start

You need the account-administration access enforced by lpsd. Preserve a
working administrative sign-in route when changing administrative
membership or credentials.

## Inspect and change an account

1. Open **Principals Manager**, choose **Users** or **Groups**, and use
   **Find** to locate the identity.
2. Select it and check its security identifier, local or built-in status,
   memberships, and sign-in configuration.
3. Choose the intended account operation and review its confirmation.
   Change only the membership, claim, credential, or sign-in setting
   required for the task.
4. Read lpsd's response. If it refuses a change that would leave no enabled
   administrator able to sign in, retain or repair another valid route
   before reconsidering the operation.

## Check the result

Refresh with **F5** and verify the stored identity and membership. Test a
new sign-in when the change concerns credentials or token policy.
Existing sessions retain their own tokens.

Use **Privileges** for registry-based local sign-in policy. Its preview
describes what a new sign-in would receive, not the rights of every
already running process. For changing only your own display name,
password, keys, or locale, use My Settings instead.

## Choosing an account action

Account actions have different effects:

| Action | Effect |
|---|---|
| Edit | Changes the full name, home path, shell, or primary group. Changing the home path does not move files. |
| Rename | Changes the name while retaining the SID and existing access grants. |
| Set password | Changes the password without ending existing sessions. |
| Sign-in | Selects accepted credentials and permitted kinds of sign-in. |
| Disable | Refuses new sign-ins while preserving the account and its SID. |
| Delete | Removes the account permanently; its old SID is not reassigned. |

Adding an SSH key does not itself enable SSH-key authentication. Check
**Sign-in** as well. Credential-free sign-in does not permit SSH access.
Explicit permission to run a service is separate from ordinary sign-in
methods. An empty claim and an absent claim have different meanings to
access conditions; inspect the intended condition before removing one.

A group rename retains its SID. A user's primary-group membership cannot
be removed until another primary group is assigned. Built-in groups with
stored membership can have members added or removed, but their system
identity cannot be renamed or deleted.

Disabling an account does not end its existing sessions. Use
[Task Manager](~peios/task-manager#sessions-and-visibility) to request
sign-out when required, after accounting for unsaved work. The account
store's last-administrator safeguard does not prove that a console or
network sign-in route is reachable. Test fresh access before closing your
working administrative session.

## Creating an account

To create a local user:

1. Select **Users**, then **New user**, or press **Ctrl+N**.
2. Enter the sign-in name and optional full name. Set and confirm a
   password, or explicitly select credential-free sign-in where permitted.
3. Select administrator membership only when required. Check the proposed
   home path and shell before selecting **Create**.
4. Read the result and verify the new user's SID and configuration. A
   refused creation does not leave a partially configured user.

To create a group, select **Groups**, then **New group**, and provide its
name and description. Add members after verifying the new group's SID.
