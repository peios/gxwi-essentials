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

Build on x86-64 Linux after preparing the explicit Peios SDK described in
[Building and Checking GXWI Essentials](docs/building.md):

```sh
./bazel build //:artifacts
./bazel test //...
./bazel build //:packages
```

The package store contains separate runtime and debugging packages for each
application, plus a suite source package. Build outputs are unsigned and
marked `release_ready: false`. Host tests and native userspace checks are
separate from installed-desktop qualification.

Read the [user guides](docs/learn/), [manuals](docs/man/), and the two registry
references in `docs/`. Peios Learn merges the Learn contribution into the
shared Peios documentation. The [build guide](docs/building.md) covers contributor
checks, private dependency access, offline native builds, and CI.
