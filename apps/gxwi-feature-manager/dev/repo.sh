#!/bin/sh
# Runs on the host: give the VM that ../gxwi/dev/boot.sh started features to
# turn on and off. It packages the example feature here and dynamic-boot from
# ../pkgs (with its feature.toml, before a release has it), publishes them in
# a small repository on the share signed with the development key, adds it
# in the guest and installs both.
#
#     dev/repo.sh          build it if it isn't there, add it and install
#     dev/repo.sh --new    build it again
set -eu
cd "$(dirname "$0")/.."
share=../gxwi/target/vmshare
repo=$share/featrepo
key=../pkgs/dev-signing.key
[ -d "$share" ] || { echo "no $share: boot the VM from ../gxwi first" >&2; exit 1; }
[ "${1:-}" = --new ] && rm -rf "$repo"
if [ ! -d "$repo" ]; then
    tool=$(mktemp -d)
    trap 'rm -rf "$tool"' EXIT
    (cd ../peipkg && go build -o "$tool/peipkg-repo" ./cmd/peipkg-repo)
    # The development keyring signs each package with the key the
    # repository is signed with.
    keyring=$(cd ../pkgs && pwd)/dev.keyring.pekit.toml
    (cd dev/example-feature && pekit package --no-gates --keyring "$keyring")
    (cd ../pkgs/dev.peios.feat-dynamic-boot && pekit package --no-gates --keyring "$keyring")
    example=$(ls -t dev/example-feature/out/package/*/dev.peios.feat-example_*.peipkg | head -1)
    dynamic=$(ls -t ../pkgs/dev.peios.feat-dynamic-boot/out/package/*/dev.peios.feat-dynamic-boot_*.peipkg | head -1)
    "$tool/peipkg-repo" init "$repo" --name peios-dev-features --key "$key" \
        --description "Development features for trying Feature Manager" > /dev/null
    "$tool/peipkg-repo" publish "$repo" "$example" "$dynamic" --key "$key"
fi
anchor=$(sed -n 's/.*"fingerprint": "\([0-9a-f]*\)".*/\1/p' "$repo/repo.json" | head -1)
# Resolving needs every configured repository trusted, the medium's too,
# whose trust ceremony a fresh boot may not have run yet.
../gxwi/dev/guest.sh "peipkg repo add peios-dev-features file:///share/featrepo --anchor $anchor
peipkg repo add peios-medium
peipkg install --yes --allow-stale dev.peios.feat-example dev.peios.feat-dynamic-boot; feat list"
