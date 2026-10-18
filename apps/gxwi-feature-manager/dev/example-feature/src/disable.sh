#!/bin/sh
# example / disable — clear On.
set -eu
echo "example: turning off"
sleep 2
reg set "Machine/Software/Feature Example" On dword:0
echo "example: off"
