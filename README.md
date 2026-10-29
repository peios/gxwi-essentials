# GXWI Essentials

GXWI Essentials contains 18 desktop applications and shared dialog helpers
for Peios. It covers files, a terminal, text editing, accounts, permissions,
services, software, networking, storage, settings, and recorded events.
The GXWI platform, desktop shells, and installer are separate projects.

Applications live under `apps/`; shared filesystem and process helpers are
under `crates/`. First-party applications and helpers share the suite's
0.1.0 development version. Applications remain separately installable.
Vendored dependencies retain their own versions and provenance.

The file explorer, terminal, and text editor are named
`gxwi-file-explorer`, `gxwi-terminal`, and `gxwi-text-editor`. Their earlier
names were Gexora, Peiterm, and Peitor. The session clipboard filename
`gexora.held` remains unchanged for interoperability.

Build and test on x86-64 Linux with pekit, as
[Building and Checking GXWI Essentials](docs/building.md) describes:

```sh
pekit build
pekit test
```

Host tests are separate from installed-desktop qualification.

Read the [user guides](docs/learn/), [manuals](docs/man/), and the two registry
references in `docs/`. Peios Learn merges the Learn contribution into the
shared Peios documentation. The [build guide](docs/building.md) covers contributor
checks, private dependency access, how the build is planned, and test disks.
