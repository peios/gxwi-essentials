#!/bin/sh
# example / install — make its key, with On off.
set -eu
echo "example: making Machine\\Software\\Feature Example"
sleep 2
reg apply - <<'EOF'
{ "keys": [ { "path": "Machine/Software/Feature Example", "values": [
  { "name": "On", "type": "dword", "data": 0 }
] } ] }
EOF
sleep 1
echo "example: installed"
