# Sourced from the gxwi-feature-manager root, not run: what cargo needs to
# build libgxwi, and feat in ../peiosutils, against sibling checkouts.
REPO="$(cd .. && pwd)"
export BINDGEN_EXTRA_CLANG_ARGS="-isystem $(gcc -print-file-name=include)"
export PEIOS_LIB_DIR="$REPO/libpeios/target/debug"
export PEIOS_INCLUDE="$REPO/libpeios/include"
export PKM_UAPI="$REPO/pkm/out/build/headers/usr/include"
export LD_LIBRARY_PATH="$REPO/libpeios/target/debug"
