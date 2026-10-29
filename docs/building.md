# Building and Checking GXWI Essentials

Use pekit to build the suite's 18 applications and to run their tests.
Pekit builds each crate on its own, so a change rebuilds only the crates it
reaches. The supported development host is x86-64 Linux. Host tests and
native Peios checks cover different environments; neither replaces a live
desktop qualification run.

## Before you start

You need pekit and Git. Everything else is pinned and fetched by pekit on
the first build: the Bootlin C toolchain, Rust 1.98.1 with Clippy, Python
3.14, busybox, Node.js 22, libclang, and the crates `Cargo.lock` selects.
The first build needs network access; later builds find what they need in
pekit's store, and every step runs without network access in a sandbox
that holds only its declared inputs.

The build files use the gcslib libraries named in `workspace.gcsl` and the
build files, the libraries of peios-rs and the Peios SDK, and the libraries
of the repositories whose client crates the applications use: gxwi, netd,
resolvd, timed, trustd, and peiosutils. `pekit lock` pins their commits in
`pekit.lock`. The `gxwi` repository is private; configure read access to it
before the first build.

The `vendor/kernel` subset retains exact upstream source hashes and its
origin record. Its three crates are Cargo workspace members for local
build inputs, but keep their upstream versions. Only the suite's owned
applications and helpers share its development version.

## Build

Build one application by its target, the crate's name with underscores:

```sh
pekit build gxwi_file_explorer
```

Each application is a target: `gxwi_disk_manager`, `gxwi_event_viewer`,
`gxwi_feature_manager`, `gxwi_file_dialog`, `gxwi_file_explorer`,
`gxwi_my_settings`, `gxwi_network_manager`, `gxwi_package_manager`,
`gxwi_principals_manager`, `gxwi_registry_editor`, `gxwi_sd_editor`,
`gxwi_security_policy`, `gxwi_services_manager`, `gxwi_system_settings`,
`gxwi_task_manager`, `gxwi_terminal`, `gxwi_text_editor`, and
`gxwi_upgrade_peios`. Build them all with `pekit build`.

Pekit prints where each output is, under `pekit-out/<target>/`. The
programs load `libpeios.so.0`, and the client libraries their crates link,
by their shared object names (SONAMEs); they carry no runtime search path.

Opening a program requires a running GXWI desktop with the appropriate
Peios services. Tests do not create that session or exercise privileged
management actions on your host.

## Test

```sh
pekit test
```

| Test | Check |
|---|---|
| `units` | Each member's unit tests |
| `doctests` | Each member's documentation tests |
| `clippy` | Clippy's checks of every target of every member, with every warning denied |
| `terminal_page` | Node.js's test runner checks Terminal's page input handling in `apps/gxwi-terminal/page/input.test.mjs` |
| `kernel_vendor` | The vendored kernel sources match the export `vendor/kernel/source-sha256.json` records |

The unit tests are given libpeios and each client library, and busybox's
`/bin/sh` and tools, which some run. A test whose inputs did not change is
found in the store and not run again.

For the full contributor check, install the pinned PSC tools into a
directory outside the repository, then run the shared entry point:

```sh
python3 tools/prepare-style-tools.py "$HOME/.cache/gxwi-essentials-style"
export PEIOS_STYLE_BIN="$HOME/.cache/gxwi-essentials-style/bin"
tools/check
```

`tools/check` checks owned source formatting and lint, then runs
`pekit test`. The tool versions and hashes are recorded in
`tools/style-tools.lock.json`. Imported sources retain their original
formatting and provenance. GitHub's workflow runs the style checks alone;
builds and tests run with pekit.

## How the build is planned

`build.gcsl` plans the Cargo workspace with the gcslib rust library's
`workspace`: Cargo works out each member's build, unit by unit, and pekit
compiles each unit as a step. Planning reads the manifests, `Cargo.lock`,
and which files exist, never the sources, so editing a source plans nothing
again.

Keep Rust dependency versions in the Cargo manifests and `Cargo.lock`, and
update the lockfile deliberately with Cargo. `peios-sys` generates its
bindings from the SDK's headers with bindgen and links libpeios, so it, and
every crate that depends on it, is given what peios-rs's library says it
needs: libpeios and its headers, built from the Peios SDK's own files.
Each client crate the applications use links its repository's C library:

| Crate | Library | Repository |
|---|---|---|
| `gxwi` | libgxwi | gxwi |
| `netd-client` | libnetd | netd |
| `resolvd-client` | libresolvd | resolvd |
| `timed-client` | libtimed | timed |
| `trust-client` | libtrust | trustd |
| `peios-reg`, `peios-regman` | libreg, libregman | peiosutils |

Each repository's library gives its crate the C library, built from that
repository's own files, and gives the tests the library to load. The rust
library fails a build whose `Cargo.lock` takes a repository's crates at
another commit than `pekit.lock` pins its library at, so each crate and the
library it links always agree. Update both together: change the commit in
`Cargo.toml`, run Cargo to update `Cargo.lock`, then remove the
repository's entry from `pekit.lock` and run `pekit lock`.

## Prepare disposable disks

To inspect Disk Manager against varied storage, create images in a new,
absolute directory outside the repository:

```sh
tools/prepare-test-disks /tmp/gxwi-test-disks > /tmp/gxwi-test-disks.args
```

Install `sgdisk` (gdisk), `mkfs.vfat` (dosfstools), and `mke2fs` (e2fsprogs)
on the host first. The helper creates an 8 GiB sparse fixed disk with a
512 MiB FAT EFI partition, a 4 GiB ext4 partition, a 1 GiB unformatted
partition, and remaining free space. A second 2 GiB sparse image contains
a FAT partition for USB-storage testing. Actual host storage use grows as
the filesystems and guest write data.

The helper formats only the new regular image files. It does not mount
images, attach loop devices, or write to host block devices, and needs no
root privileges. It refuses an existing output directory, including a
symlink. The parent directory must exist; output paths can contain letters,
digits, dots, slashes, underscores, and hyphens. On failure, inspect the
partial directory and choose a fresh path for another attempt.

Standard output contains QEMU arguments attaching the images as a virtio
fixed disk and removable USB storage. A GXWI VM launcher that accepts
`VM_EXTRA` can consume the saved arguments:

```sh
GXWI_VM_RUNNER=/absolute/path/to/drive.py \
    VM_EXTRA="$(cat /tmp/gxwi-test-disks.args)" /path/to/gxwi/dev/boot.sh
```

Set `GXWI_VM_RUNNER` to the selected image runner's absolute path. The
runner and image must match the desktop stack being qualified.

Reuse that argument file to retain guest changes. For fresh fixtures, run
the helper with a different directory. These images support viewing and
external guest experiments; Disk Manager's mutation backend remains
unavailable.

The old Feature Manager, Package Manager, and Upgrade Peios repository
fixtures used Pekit archives, development signing keys, and the legacy
`peipkg` repository and trust workflow. They are retired with that build
path; their source remains in the imported Git history and migration
backups. The unsigned PPK store built here does not replace those installer,
feature-lifecycle, rollback, or release-upgrade qualification scenarios.
Those checks need a compatible disposable guest and repository workflow;
the applications still call their existing runtime services and commands.

## Not built with pekit yet

Packages, debug information, and builds with the native Peios toolchain,
which hardens programs with control-flow enforcement technology (CET), are
not yet built with pekit. The package definitions and native build settings
from the Bazel build are kept outside this repository until they are.

The old Feature Manager, Package Manager, and Upgrade Peios repository
fixtures used Pekit archives, development signing keys, and the legacy
`peipkg` repository and trust workflow. They are retired with that build
path; their source remains in the imported Git history. Those
installer, feature-lifecycle, rollback, or release-upgrade qualification
scenarios need a compatible disposable guest and repository workflow; the
applications still call their existing runtime services and commands.

Installed-desktop qualification needs a compatible Peios kernel, GXWI
session, registry and service access, and deliberate checks of each
application's operations. Record those results separately. In particular,
Disk Manager's mutation backend is unavailable, and Network Manager labels
its example firewall displays; a successful build does not remove those
limitations.

## Check documentation

Use Trail revision `1d662e98cbcbdd5c51c01533afcab7f5723a2eff` or a compatible
version with recursive-merge support:

```sh
TRAIL=/absolute/path/to/trail tools/check-docs
```

The check lints `docs/learn/`, builds it strictly into a temporary directory,
and removes that output. It does not start desktop applications or execute
management procedures. Panorama merges the suite's `peios.product` into its
shared Peios tree. Check the combined documentation when changing links or
hierarchy, keeping generated output outside the source tree.
