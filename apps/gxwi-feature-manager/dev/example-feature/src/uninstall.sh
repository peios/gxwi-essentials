#!/bin/sh
# example / uninstall — delete its key, if it is there.
set -eu
echo "example: deleting Machine\\Software\\Feature Example"
sleep 2
reg del -r -y "Machine/Software/Feature Example" || true
echo "example: removed"
