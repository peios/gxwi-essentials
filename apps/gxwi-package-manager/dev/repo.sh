#!/bin/sh
# Runs on the host: give the VM that ../gxwi/dev/boot.sh started something
# to install, update and remove. It builds a small repository on the share
# from packages ../pkgs has published, signed with the development key they
# are signed with, and adds it in the guest beside the medium's.
#
# It holds a few small tools the image doesn't install, and two versions of
# dev.peios.net, so that the image's 0.1.5 has an update and the update can
# be undone. which is installed from it, for a package with a repository.
#
#     dev/repo.sh          build it if it isn't there, and add it
#     dev/repo.sh --new    build it again
set -eu
cd "$(dirname "$0")/.."
share=../gxwi/target/vmshare
repo=$share/devrepo
pool=../pkgs/_repo2_/p
key=../pkgs/dev-signing.key
[ -d "$share" ] || { echo "no $share: boot the VM from ../gxwi first" >&2; exit 1; }
[ "${1:-}" = --new ] && rm -rf "$repo"
if [ ! -d "$repo" ]; then
    tool=$(mktemp -d)
    trap 'rm -rf "$tool"' EXIT
    (cd ../peipkg && go build -o "$tool/peipkg-repo" ./cmd/peipkg-repo)
    "$tool/peipkg-repo" init "$repo" --name peios-dev --key "$key" \
        --description "Development packages for trying Package Manager" > /dev/null
    # The older version first, so that the newer one archives it.
    "$tool/peipkg-repo" publish "$repo" "$pool"/dev.peios.net/0.1.5-1/*.peipkg --key "$key"
    set --
    for name in dev.peios.net org.gnu.make org.gnu.cpio org.gnu.cpio-common org.gnu.patch org.gnu.bc \
        org.gnu.which org.gnu.which-common org.debian.netbase com.facebook.zstd-utils org.tukaani.xz-utils; do
        latest=$(ls -d "$pool/$name"/*/ | sort -V | tail -1)
        set -- "$@" "$latest"*.peipkg
    done
    "$tool/peipkg-repo" publish "$repo" "$@" --key "$key"
fi
anchor=$(sed -n 's/.*"fingerprint": "\([0-9a-f]*\)".*/\1/p' "$repo/repo.json" | head -1)
# The image has no grep: what is already there is asked of peipkg. Adding a
# repository that is there runs its trust ceremony again, which is harmless.
../gxwi/dev/guest.sh "peipkg repo add peios-dev file:///share/devrepo --anchor $anchor
peipkg info org.gnu.which > /dev/null 2>&1 || printf '%s\n' '{\"id\":1,\"answer\":\"yes\"}' | peipkg --driven install org.gnu.which --allow-stale"
