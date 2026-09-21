#!/usr/bin/env sh
# Install toolwall into a throwaway config directory, so you can walk through
# a first run without touching the one you actually use.
#
# Your real ~/.config/waywall is never opened. The cargo binaries are shared,
# because building a second copy of them to look at a window would be silly.
#
#   tools/sandbox.sh                 a machine that just installed waywall
#   tools/sandbox.sh --gore          one that already runs gore's config
#   tools/sandbox.sh --mine          a copy of your real config
#   tools/sandbox.sh --clean         delete the sandbox
#
#   TOOLWALL_SANDBOX=<dir>           somewhere other than /tmp/toolwall-sandbox
set -eu

SRC="$(cd "$(dirname "$0")/.." && pwd)"
BOX="${TOOLWALL_SANDBOX:-/tmp/toolwall-sandbox}"
REAL="${XDG_CONFIG_HOME:-$HOME/.config}/waywall"

SEED="empty"
for arg in "$@"; do
    case "$arg" in
        --gore) SEED="gore" ;;
        --mine) SEED="mine" ;;
        --empty) SEED="empty" ;;
        --clean)
            rm -rf "$BOX"
            echo "Removed $BOX"
            exit 0
            ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

rm -rf "$BOX"
mkdir -p "$BOX/config" "$BOX/data" "$BOX/state"

case "$SEED" in
empty)
    # Nothing at all, which is what waywall leaves you with. It never makes
    # this directory, so a new install really does look like this.
    echo "Starting from: a machine that just installed waywall"
    ;;

gore)
    command -v git >/dev/null 2>&1 || { echo "--gore needs git" >&2; exit 1; }
    echo "Starting from: gore's generic config"
    git clone --depth 1 -q https://github.com/arjuncgore/waywall_generic_config \
        "$BOX/config/waywall"
    rm -rf "$BOX/config/waywall/.git"
    ;;

mine)
    [ -d "$REAL" ] || { echo "no config at $REAL" >&2; exit 1; }
    echo "Starting from: a copy of $REAL"
    mkdir -p "$BOX/config/waywall"
    cp -r "$REAL/." "$BOX/config/waywall/"
    # A copy of a toolwall install is not a waywall config to import, so put
    # it back the way it was before toolwall got to it.
    if [ -f "$BOX/config/waywall/init.lua.pre-toolwall" ]; then
        mv "$BOX/config/waywall/init.lua.pre-toolwall" "$BOX/config/waywall/init.lua"
        rm -f "$BOX/config/waywall/toolwall.json"
    fi
    ;;
esac

echo
XDG_CONFIG_HOME="$BOX/config" XDG_DATA_HOME="$BOX/data" XDG_STATE_HOME="$BOX/state" \
    TOOLWALL_NO_WAYWALL_ICON=1 \
    sh "$SRC/install.sh" < /dev/null

cat <<EOF

---------------------------------------------------------------------------
The sandbox is at $BOX

Open the setup window against it:

  XDG_CONFIG_HOME=$BOX/config toolwall-gui --setup

Look at what it wrote:

  cat $BOX/config/waywall/toolwall.json
  cat $BOX/config/waywall/toolwall-import-report.txt

Throw it away:

  tools/sandbox.sh --clean

Your own config at $REAL was not touched.
---------------------------------------------------------------------------
EOF
