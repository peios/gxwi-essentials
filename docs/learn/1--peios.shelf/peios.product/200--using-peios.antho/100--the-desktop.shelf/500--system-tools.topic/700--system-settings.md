---
title: Changing System Settings
type: how-to
description: Change a machine setting and check its application timing.
---

System Settings changes machine-wide time, locale, startup, and identity
settings. Personal account choices belong to My Settings, and shell
appearance belongs to Desktop Settings.

## Before you start

You need access to write the relevant registry key or configuration file.
Some immediate operations also require a daemon control right. Record the
existing setting before a change that needs a restart or affects access.

## Change a setting

1. Open **System Settings** and choose the relevant section: **Time &
   Date**, **Language & Keyboard**, **Startup & Shutdown**, or **About**.
2. Inspect the current value and its scope. A switch or list choice
   applies when selected; typed values use the associated **Apply**,
   **Save**, or **Set Clock** action.
3. Make one intended change and read the status line. A refused immediate
   choice is put back.

## Check the result

The window rereads state every five seconds. Verify the consuming service
or the next session as appropriate, not only the stored field.

Time settings are read by timed; setting the clock manually needs its
control right and automatic time disabled. Console keyboard changes try to
restart the keymap service when permitted; otherwise they take effect at
boot. Machine locale defaults affect new sessions. The machine name is
applied by netd.

Startup settings include peinit stage timing, console automatic sign-in,
and selected kernel-command-line options when dynamic boot is available.
Changing the next boot image's inputs does not change the running kernel.
See `gxwi-system-settings(1)` and each setting's registry manual.

The language and format selections save `LANG` with the time, number, money,
measurement, and paper categories. Use `reg` when changing one independently
configured category. For personal precedence and new-session checks, follow
[Choosing Session Locales](~peios/session-settings/session-locales).
