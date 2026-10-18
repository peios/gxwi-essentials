#!/bin/sh
# example / enable — set On, or fail on purpose while the marker file exists.
set -eu
echo "example: turning on"
sleep 2
if [ -e /tmp/feat-example-fail ]; then
    echo "example: /tmp/feat-example-fail exists, so this fails on purpose" >&2
    exit 3
fi
reg set "Machine/Software/Feature Example" On dword:1
echo "example: on"
