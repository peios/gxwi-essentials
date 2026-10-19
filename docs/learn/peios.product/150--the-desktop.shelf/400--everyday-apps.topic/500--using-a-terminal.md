---
title: Using a Terminal
type: how-to
description: Open a shell or a terminal program in your desktop session.
---

Terminal runs a shell or another program on a pseudoterminal inside the
desktop. The running terminal belongs to the application, so a browser
reconnection shows the terminal state that remains on the machine.

## Before you start

You need Terminal installed and a running GXWI session. Commands run with
your account's permissions and can change files or system settings within
those permissions.

## Open and use the terminal

1. Open **Terminal** from the launcher. It starts your configured absolute
   shell path, or `/bin/sh`, in your home directory.
2. Enter a command and press **Enter**. Programs that need a desktop
   connection inherit the session environment.
3. Use **Ctrl+Shift+C** to copy selected text and **Ctrl+Shift+V** or
   **Shift+Insert** to paste.

## Check the result

Read the program's output and exit status as you would in another
terminal. Ending the shell closes its window. Closing the application
window hangs up the terminal, which can terminate work still running in
it. Closing a browser page is a different operation from closing the
application window.

To open a shell in a folder, use the folder's application menu in File
Explorer. The command interface also accepts one absolute directory path,
or a program followed by its arguments; see `gxwi-terminal(1)`.

The desktop retains its own keyboard shortcuts while the terminal has
focus. A shortcut reserved by the desktop does not become input to the
terminal program.

## When input waits

Terminal keeps up to 1 MiB of pending input in each browser page. It sends
that input in order as the running program accepts it. If the buffer fills,
typing pauses while the program catches up; its output continues to appear.
A paste that does not fit the remaining space is refused in full with a
message. Wait for the program to read, or paste a smaller amount.

If the connection breaks with input pending, Terminal warns that some input
might already have reached the program. It does not replay that input after
reconnecting, because replaying a command could run it twice. Input already
received by the application might still reach the program. Check the output
before deciding what to type again. The warning remains after reconnection
until you start entering new input.
