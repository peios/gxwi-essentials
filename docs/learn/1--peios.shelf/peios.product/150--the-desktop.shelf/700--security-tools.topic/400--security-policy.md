---
title: Managing Certificate Trust
type: how-to
description: Inspect a certificate before adding it to machine trust.
---

Security Policy manages the certificate authorities the machine accepts
and its explicit distrust entries. A trust change affects programs that
consume that machine policy, so inspect the certificate and its source
before accepting it.

## Before you start

You need permission to change `Machine\System\Trust`. Reloading trustd
uses its separate control permission. Obtain the certificate or fingerprint
through the verification process appropriate to the authority you trust.

## Inspect and change trust

1. Open **Security Policy** and select **Trusted** or **Distrusted**.
2. Inspect the issuer, validity, fingerprint, and trust purpose. To add a
   certificate, use **Add Certificate** and select its PEM or DER file.
3. Review the parsed certificate. The application rejects a non-authority
   certificate, an expired certificate, and conflicting existing trust or
   distrust entries.
4. Confirm only the intended trust, distrust, or removal operation. Read
   the result.

## Check the result

Verify the certificate in the effective list read from trustd. The window
follows changes made through other tools too. A stored entry alone does
not establish that every consuming program has refreshed its state.

**Settings** shows trustd's status, its reload operation, certificate-file
export settings, and permissions. **Signing Keys** displays code-signing
keys compiled into the kernel; this window cannot add or remove those
keys. See `gxwi-security-policy(1)` and `trust(1)`.
