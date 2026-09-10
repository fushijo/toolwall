#!/usr/bin/env sh
# Apply the toolwall patches to a waywall checkout and rebuild it.
#
# Usage: patches/apply.sh [path-to-waywall-checkout]
#
# Everything toolwall needs beyond stock waywall lives in this directory. The
# patches are small and additive; see README.md next to this script for what
# each one does and why it is not just a Lua workaround.
set -eu

PATCHES="$(cd "$(dirname "$0")" && pwd)"
SRC="${1:-$HOME/waywall}"

if [ ! -f "$SRC/meson.build" ]; then
    echo "no waywall checkout at $SRC" >&2
    echo >&2
    echo "Clone one and pass it in:" >&2
    echo "  git clone https://github.com/tesselslate/waywall ~/waywall" >&2
    echo "  $0 ~/waywall" >&2
    exit 1
fi

cd "$SRC"

for patch in "$PATCHES"/*.patch; do
    name="$(basename "$patch")"

    if git apply --reverse --check "$patch" >/dev/null 2>&1; then
        echo "already applied: $name"
        continue
    fi

    if ! git apply --check "$patch" >/dev/null 2>&1; then
        echo "cannot apply $name to this waywall revision" >&2
        echo "It was written against $(head -1 "$patch" | cut -c1-60)..." >&2
        echo "Check out a closer revision, or apply it by hand." >&2
        exit 1
    fi

    echo "applying: $name"
    git apply "$patch"
done

if [ ! -d build ]; then
    echo "configuring build"
    meson setup build
fi

echo "building"
ninja -C build

echo
echo "Built $SRC/build/waywall/waywall"
echo "Installing needs root, so this will ask for your password:"
echo
sudo ninja -C build install

echo
echo "Done. Restart waywall to pick it up."
