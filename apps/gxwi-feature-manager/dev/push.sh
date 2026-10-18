#!/bin/sh
# Runs on the host: build Feature Manager and put it in the share of the VM
# that ../gxwi/dev/boot.sh started, with its icon for the base theme and its
# declaration for the catalogue. GXWI's dev service puts them where a package
# would, and its dev loop restarts on a new one.
#
# The feat it drives goes on the share too, built on its own from
# ../peiosutils, until a peiosutils with the driven mode is in the image.
#
# The rename makes the new binary appear whole, never half-written. Both link
# libpeios, so they are ordinary glibc builds and use the libpeios the guest
# image ships.
set -eu
cd "$(dirname "$0")/.."
. dev/env.sh
[ -d ../gxwi/target/vmshare ] || { echo "no ../gxwi/target/vmshare: boot the VM from ../gxwi first" >&2; exit 1; }
share=$(cd ../gxwi/target/vmshare && pwd)
cargo build --release
(cd ../peiosutils && cargo build --release -p pu_feat --bin feat)
cp ../peiosutils/target/release/feat "$share/feat.new"
mv "$share/feat.new" "$share/feat"
mkdir -p "$share/icons/base"
cp gxwi-feature-manager.svg "$share/icons/base/dev.peios.gxwi-feature-manager.svg"
mkdir -p "$share/apps"
cp dev.peios.gxwi-feature-manager.toml "$share/apps/dev.peios.gxwi-feature-manager.toml"
cp target/release/gxwi-feature-manager "$share/gxwi-feature-manager.new"
mv "$share/gxwi-feature-manager.new" "$share/gxwi-feature-manager"
