---
title: Principals Manager
type: concept
description: Distinguish local accounts, built-in identities, and sign-in policy.
---

Principals Manager displays identities the machine knows and administers
local users and groups through lpsd. Built-in identities such as SYSTEM
and Administrators are defined by the system; they are not ordinary local
accounts to recreate or delete.

**Users** and **Groups** show names, identifiers, membership, and available
details. A user's page includes permitted sign-in methods, claims, and
administrative credential information where access allows it.

Local-account changes go through lpsd. The backend enforces its rules,
including retaining an enabled administrator that can sign in. The window
reports a refusal rather than bypassing that safeguard.

**Privileges** edits local sign-in policy in the registry. Those records
can set privileges, integrity, owner, and a default discretionary access
control list. A user's page previews the combined policy for a new
sign-in. Changing that policy is separate from editing the local account,
and uses the policy key's write access.

Account administration requires the enabled Administrators group in the
requester's token. Policy writes require the registry rights instead.
The interface shows read-only data when it can inspect an object but
cannot change it. Changes to sign-in policy apply to new sign-ins, not
retroactively to existing tokens.

See [Managing Local Accounts](~peios/managing-local-accounts) and
`gxwi-principals-manager(1)`. The account authority and shared sign-in
semantics remain documented with authd and lpsd.

## Reviewing local policy

**Privileges** lists local policy records and a **Denied to everyone**
entry. A denied privilege cannot be granted by another record. A user's
policy preview combines the records for that user, their groups, and
Everyone; select the intended kind of sign-in before interpreting it.

**New record** selects a principal. **Edit** changes its privileges,
integrity, owner, or default DACL. An empty field makes no assignment for
that field. **Delete** removes the record; it does not imply a deny if
another applicable record still grants access. Default DACL text must be
valid SDDL understood by authd.

The editor warns before removing `SeChangeNotifyPrivilege` from Everyone
or Administrators because losing directory-traversal privilege can prevent
shell startup. On a machine with no policy, authd's default gives Everyone
that privilege. The editor preserves this default when it creates the
first policy, rather than silently discarding it.

Policy warnings above the list identify records authd ignores or duplicate
principal assignments. Resolve these using the shared account-policy
reference. Refresh with **F5** after another administrator changes an
account or policy record.
