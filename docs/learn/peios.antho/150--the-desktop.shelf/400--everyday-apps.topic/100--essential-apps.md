---
title: Essential Desktop Applications
type: concept
description: Choose the desktop tool for a file, account, or system task.
---

GXWI Essentials is a suite of desktop applications for everyday work and
system administration. Each application is installed separately. They run
inside your GXWI session and use your account's access rights; opening an
administration window does not give it additional authority.

The suite supplies applications, not the desktop compositor or shell.
Fenesh and Fenestra provide separate desktop interfaces. Their launchers
can open the applications installed on the machine.

## Choosing an application

Use the application whose scope matches the task:

| Task | Application |
|---|---|
| Browse, copy, move, or remove files | File Explorer |
| Edit a small text file | Text Editor |
| Run a shell or terminal program | Terminal |
| Inspect running processes and sessions | Task Manager |
| Control or define a service | Services Manager |
| Inspect disks and mounts | Disk Manager |
| Configure interfaces, name resolution, or firewall rules | Network Manager |
| Inspect registry keys and values | Registry Editor |
| Change machine time, locale, startup, or name | System Settings |
| Install, update, or remove packages | Package Manager |
| Enable or disable installed feature definitions | Feature Manager |
| Upgrade the Peios release | Upgrade Peios |
| Change your own account settings | My Settings |
| Administer local users, groups, and privileges | Principals Manager |
| Manage certificate trust | Security Policy |
| Read events, logs, and metrics | Event Viewer |

The file dialog and permissions editor are shared dialogs opened by these
applications. They are not standalone launcher tasks.

## Reading an outcome

A displayed setting is not proof that its consuming service accepted it.
After a change, read the application's result and check the affected
service, file, or connection. A disabled control or incomplete view can
mean access was refused; it does not mean the underlying object is absent.

Two interfaces have explicit implementation limits. Disk Manager's change
forms do not modify storage. Network Manager marks several firewall
activity displays as **Example data**. Their guides describe what remains
real and what needs separate verification.

The command manuals use the executable names `gxwi-file-explorer`,
`gxwi-text-editor`, and `gxwi-terminal`, alongside the other `gxwi-*`
application names.
