#!/bin/sh
# Runs on the host: build Package Manager and put it in the share of the VM
# that ../gxwi/dev/boot.sh started, with its icon for the base theme and its
# declaration for the catalogue. GXWI's dev service puts them where a package
# would, and its dev loop restarts on a new one.
#
# The peipkg it drives goes on the share too, built from ../peipkg, until a
# peipkg with the driven mode is in the image.
#
# The rename makes the new binary appear whole, never half-written. It links
# libpeios, so it is an ordinary glibc build and uses the libpeios the guest
# image ships.
set -eu
cd "$(dirname "$0")/.."
. dev/env.sh
[ -d ../gxwi/target/vmshare ] || { echo "no ../gxwi/target/vmshare: boot the VM from ../gxwi first" >&2; exit 1; }
share=$(cd ../gxwi/target/vmshare && pwd)
cargo build --release
(cd ../peipkg && CGO_ENABLED=0 go build -o "$share/peipkg.new" ./cmd/peipkg)
mv "$share/peipkg.new" "$share/peipkg"
mkdir -p "$share/icons/base"
cp gxwi-package-manager.svg "$share/icons/base/dev.peios.gxwi-package-manager.svg"
mkdir -p "$share/apps"
cp dev.peios.gxwi-package-manager.toml "$share/apps/dev.peios.gxwi-package-manager.toml"
cp target/release/gxwi-package-manager "$share/gxwi-package-manager.new"
mv "$share/gxwi-package-manager.new" "$share/gxwi-package-manager"
