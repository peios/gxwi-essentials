---
title: Changing Your Account Settings
type: how-to
description: Update your display name, credentials, and session locale.
---

My Settings operates on your own account. Its backend identifies you from
your token rather than accepting another account name. Use Principals
Manager for administrative changes to other accounts.

## Before you start

You need a GXWI session and permission for the selected personal setting.
Changing a password or SSH key requires your current password. A key-only
account without a password cannot use those self-service forms.

## Change a personal setting

1. Open **My Settings** and choose **Account**, **Password**, **SSH Keys**,
   or **Language & Formats**.
2. For a display name, enter the new name and select **Apply**. For a
   password, enter the current password and the new password twice.
3. For an SSH key, use **Add Key** with one public-key line and your
   password, or select **Remove** for the intended key and authenticate.
4. Read the backend's outcome. Credential fields are cleared after the
   request, including on failure.

## Check the result

Inspect the resulting name or key list. Changed credentials and locale
choices affect subsequent sign-ins; they do not replace the token of an
already running session. Keep the current session available while
verifying a new credential through the intended sign-in route.

**System Default** and **Match Language** remove the relevant personal
locale override. Locale choices are written under `CurrentUser\Locale`
and apply at the next sign-in. A restricted registry key makes the
section read-only. Account state refreshes every five seconds.
See `gxwi-my-settings(1)` for interfaces and file locations.
