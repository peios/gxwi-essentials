# Building and Checking GXWI Essentials

Use x86-64 Linux, Python 3.11 or newer, a C compiler, binutils, libclang,
pkg-config, gzip, curl, Git, Node.js, npm, and Rust 1.98.1 with rustfmt and
Clippy. Groff renders the manual pages. The `bazel` launcher verifies and
caches Bazel 8.4.2. Bazel builds Rust programs directly from the dependency
lock; Cargo supplies additional contributor diagnostics.

## Prepare the SDK and dependencies

Select exact libpeios and kernel revisions and create a new SDK directory:

```sh
tools/prepare-dev-sdk /path/to/libpeios /path/to/kernel/include/uapi /tmp/gxwi-essentials-sdk
export PEIOS_SDK=/tmp/gxwi-essentials-sdk
```

The helper builds libpeios with its own Cargo interface and copies its
headers, kernel UAPI, and `libpeios.so.0` into the SDK. It does not install
on the host. Keep the selected source revisions with qualification records.
CI pins its SDK sources in `.github/workflows/check.yml`.

The locked GXWI dependency is in the private `peios/gxwi` repository.
Configure read access before fetching. When authentication uses Git's
credential helper, set `CARGO_NET_GIT_FETCH_WITH_CLI=true`. Keep credentials
outside package inputs and Git URLs.

The `vendor/kernel` subset retains exact upstream source hashes and its
origin record. Its three crates are Cargo workspace members for local
build inputs, but keep their upstream versions. Only the suite's owned
applications and helpers share its development version.

## Build and test

Build all applications and run the declared tests:

```sh
./bazel build //:artifacts
./bazel test //...
```

Build an individual application with its app label, for example:

```sh
./bazel build //apps/gxwi-file-explorer
```

Opening a program requires a running GXWI desktop with the appropriate
Peios services. Tests do not create that session or exercise privileged
management actions on your host.

Prepare the pinned PSC tools outside the repository for contributor checks:

```sh
python3 tools/prepare-style-tools.py /tmp/gxwi-essentials-style
export PEIOS_STYLE_BIN=/tmp/gxwi-essentials-style/bin
tools/check
```

The entry point checks source style and Starlark formatting, runs Bazel's
test graph, and supplements it with Cargo Clippy and library documentation
tests. Imported sources retain their original formatting and provenance.
The tools' versions and integrity hashes are recorded under `tools/`.
Use Bazel outputs as the package inputs.

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
VM_EXTRA="$(cat /tmp/gxwi-test-disks.args)" /path/to/gxwi/dev/boot.sh
```

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

## Build packages

Build the complete content-addressed package store:

```sh
./bazel build //:packages
```

The output is `bazel-bin/packages.store`. It contains canonical manifests,
compressed SHA-256 blobs, and an index for 55 packages:

- One `dev.peios.<app>` runtime package for each of the 18 applications.
- Matching `-debuginfo` and `-debugsource` packages for each application.
- `dev.peios.gxwi-essentials-source` for the suite source and build inputs.

For example, `//packages:gxwi-terminal` selects Terminal's package output.
The two dialogs retain their own runtime packages and client-library source
in the suite. Runtime package data includes applicable declarations, icons,
manuals, and registry references. Package construction does not install the
applications or apply registry settings.

Validation checks payload coverage, ELF properties, debug correspondence,
licenses, file modes, and blob identity. Host development packages do not
establish native Peios compatibility. All outputs are unsigned and marked
`release_ready: false`; constructing the store is not a release or a
publication action.

## Build in native Peios userspace

Acquire the exact dependency snapshot on a networked host with access to
the private dependency:

```sh
tools/offline-build acquire /absolute/path/gxwi-essentials-inputs
```

Use an isolated Peios userspace with its CET-enabled Rust 1.98.1 compiler
and standard library, C toolchain, Python, binutils, kernel headers, and
libpeios 0.5.10 or newer development files. The permissions editor needs
the NULL ACL corrections introduced in 0.5.10. Copy the exact suite source,
including `MODULE.bazel.lock`. Mount the acquired snapshot read-only and
provide a private writable work directory. Inside that environment:

```sh
tools/prepare-native-sdk /tmp/gxwi-essentials-native-sdk
export PEIOS_SDK=/tmp/gxwi-essentials-native-sdk
export PEIOS_RUSTC=/usr/bin/rustc
tools/offline-build test /gxwi-essentials-inputs /gxwi-essentials-work
```

Use the same wrapper with `build` for build and package outputs without
running the test command. It rejects a different module lock and disables
dependency downloads. Native package checks require CET, full RELRO,
immediate binding, and packed relocations.

Native userspace checks run on the caller's kernel. Installed-desktop
qualification additionally needs a compatible Peios kernel, GXWI session,
registry and service access, and deliberate checks of each application's
operations. Record those results separately. In particular, Disk Manager's
mutation backend is unavailable, and Network Manager labels its example
firewall displays; a successful build does not remove those limitations.

## Continuous integration

The workflow uses pinned SDK sources, Rust and Node.js versions, PSC tools,
and Trail. Configure `PEIOS_DEPENDENCIES_TOKEN` as an Actions secret with
read-only contents access to `peios/gxwi`. The default checkout token cannot
read that separate private repository. The credential helper restricts the
token to that repository and does not embed it in package data.

A missing credential produces an explicit failure. Fork pull requests that
cannot access the secret cannot fetch the private dependency. Native and
installed-desktop qualification remain separate from the hosted CI job.

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
