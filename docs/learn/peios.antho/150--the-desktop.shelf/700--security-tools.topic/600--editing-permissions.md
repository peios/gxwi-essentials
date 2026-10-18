---
title: Editing Object Permissions
type: how-to
description: Apply a descriptor change through the application that owns the object.
---

Open the permissions dialog from the application responsible for the
object. That application names its rights and applies the resulting
descriptor using the access it already holds.

## Before you start

You need access to read the relevant descriptor parts and to change the
parts you intend to edit. Record the existing owner and rules when you
need a recovery path. Application-specific rights are meaningful only in
that object's context.

## Review and apply

1. Open **Permissions** for the intended file, registry key, service, or
   policy object. Check the object name and type in the dialog.
2. Inspect the owner and existing rules. Use **Advanced Mode** when order,
   conditions, or entries outside the simple view matter.
3. Make the intended change and select **Apply** or **OK**.
4. If asked about existing children, choose propagation only for the
   intended scope and review its progress and failures.

## Check the result

Read the caller's apply response. Reopen or refresh the descriptor to
inspect what was stored, then test the intended operation with the
relevant account. Existing open handles can retain rights granted before
the descriptor changed.

An effective-access view explains a calculation from the information it
has; it is not a substitute for the consuming component's full operation
and runtime policy. Do not remove an unfamiliar advanced entry merely
because another view cannot display it.
