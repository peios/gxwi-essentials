#!/bin/sh
# Runs on the host: build peitor and put it in the share of the VM that
# ../gxwi/dev/boot.sh started, with its icon for the base theme and its
# declaration for the catalogue. GXWI's dev service puts them where a package
# would, and its dev loop restarts on a new one.
#
# The rename makes the new binary appear whole, never half-written. peitor
# links libpeios by way of libgxwi, so it is an ordinary glibc build and uses
# the libpeios the guest image ships.
#
# `--local` builds against the libgxwi beside this checkout rather than the
# one Cargo.toml names, for trying a change to both at once.
set -eu
cd "$(dirname "$0")/.."
. dev/env.sh
share=../gxwi/target/vmshare
[ -d "$share" ] || { echo "no $share: boot the VM from ../gxwi first" >&2; exit 1; }
if [ "${1:-}" = --local ]; then
    cargo build --release --config 'patch."https://github.com/peios/gxwi.git".libgxwi.path="../gxwi/libgxwi"'
else
    cargo build --release
fi
mkdir -p "$share/icons/base"
cp peitor.svg "$share/icons/base/dev.peios.peitor.svg"
mkdir -p "$share/apps"
cp dev.peios.peitor.toml "$share/apps/dev.peios.peitor.toml"
cp target/release/peitor "$share/peitor.new"
mv "$share/peitor.new" "$share/peitor"
