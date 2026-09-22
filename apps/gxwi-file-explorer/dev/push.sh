#!/bin/sh
# Runs on the host: build Gexora and put it in the share of the VM that
# ../gxwi/dev/boot.sh started, with its icon for the base theme. GXWI's dev
# service puts it in /bin, where the launcher finds it, and its dev loop
# restarts on a new one.
#
# The rename makes the new binary appear whole, never half-written. Gexora
# links libpeios by way of libgxwi, so it is an ordinary glibc build and uses
# the libpeios the guest image ships.
set -eu
cd "$(dirname "$0")/.."
. dev/env.sh
share=../gxwi/target/vmshare
[ -d "$share" ] || { echo "no $share: boot the VM from ../gxwi first" >&2; exit 1; }
cargo build --release
mkdir -p "$share/icons/base"
cp gexora.svg "$share/icons/base/dev.peios.gexora.svg"
cp target/release/gexora "$share/gexora.new"
mv "$share/gexora.new" "$share/gexora"
