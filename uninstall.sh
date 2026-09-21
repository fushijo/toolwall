#!/usr/bin/env sh
# Take toolwall back out of the waywall config directory.
#
# By default this puts back the init.lua that was there before toolwall
# replaced it. If there is no copy of it, you get a blank waywall config
# instead, because leaving the two-line toolwall shim behind after deleting
# the runtime it requires means waywall will not start at all.
#
#   --blank   blank waywall config, even if a backup exists
#   --gore    download gore's generic config and use that
#   --all     also delete toolwall.json and the import report
#   -y        do not ask
set -eu

CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/waywall"
BACKUP="$CONFIG_DIR/init.lua.pre-toolwall"
GORE_URL="https://github.com/arjuncgore/waywall_generic_config"

ALL=""
YES=""
WANT=""

for arg in "$@"; do
    case "$arg" in
        --all) ALL=1 ;;
        --blank) WANT="blank" ;;
        --gore) WANT="gore" ;;
        -y|--yes) YES=1 ;;
        -h|--help)
            sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

if [ ! -d "$CONFIG_DIR" ]; then
    echo "no waywall config directory at $CONFIG_DIR, nothing to do"
    exit 0
fi

# Work out what init.lua is going to end up being, before touching anything.
if [ -z "$WANT" ]; then
    if [ -f "$BACKUP" ]; then
        WANT="backup"
    else
        WANT="blank"
    fi
fi

if [ "$WANT" = "gore" ] && ! command -v git >/dev/null 2>&1; then
    echo "--gore needs git, which is not installed" >&2
    exit 1
fi

echo "This will remove from $CONFIG_DIR:"
echo "  toolwall.lua"
echo "  toolwall/"
echo "and the \"toolwall setup\" entry from your app menu."
echo

case "$WANT" in
    backup)
        echo "init.lua goes back to the copy made when toolwall was installed."
        ;;
    blank)
        if [ -f "$BACKUP" ]; then
            echo "init.lua becomes a blank waywall config. Your backup at"
            echo "init.lua.pre-toolwall is left where it is."
        else
            echo "init.lua becomes a blank waywall config: there is no"
            echo "init.lua.pre-toolwall to go back to."
        fi
        ;;
    gore)
        echo "gore's generic config gets downloaded from"
        echo "  $GORE_URL"
        echo "and installed here. Anything it would overwrite is moved aside"
        echo "to <name>.before-gore first. It is a few tens of megabytes: the"
        echo "repository ships the Ninjabrain Bot and paceman jars."
        ;;
esac

if [ -n "$ALL" ]; then
    echo
    echo "--all: toolwall.json and toolwall-import-report.txt go too."
else
    echo
    echo "toolwall.json is kept. Pass --all to delete it as well."
fi

if [ -z "$YES" ] && [ -t 0 ]; then
    printf "Go ahead? [y/N] "
    read -r reply
    case "$reply" in
        y|Y|yes|YES) ;;
        *) echo "nothing done"; exit 0 ;;
    esac
fi

echo

# Writing to a symlink writes through it, the same way installing did. If
# init.lua points into a dotfiles repo, that is the file being rewritten.
if [ -L "$CONFIG_DIR/init.lua" ]; then
    echo "Note: init.lua is a symlink to $(readlink "$CONFIG_DIR/init.lua")"
    echo "      That is the file this writes to."
    echo
fi

# ---------------------------------------------------------------------------
# Put a config back.
#
# This happens before the runtime is deleted. If it is going to fail it should
# fail while init.lua still has something that works underneath it.
# ---------------------------------------------------------------------------

case "$WANT" in
backup)
    cp "$BACKUP" "$CONFIG_DIR/init.lua"
    echo "Restored init.lua from init.lua.pre-toolwall"

    # The copy has done its job, and leaving it is not harmless: install.sh
    # only takes a backup when there is not one already, so a stale copy here
    # means the next real config never gets saved.
    rm -f "$BACKUP"
    ;;

blank)
    cat > "$CONFIG_DIR/init.lua" <<'LUA'
--[[
    A blank waywall config.

    Everything is optional: waywall runs on `return {}`. Add what you want
    under these tables, or drop someone else's config in here instead.

    https://github.com/tesselslate/waywall/blob/main/doc/01_options.md
]]

local config = {
    input = {
        layout = "",
        sensitivity = 1.0,
    },
    theme = {
        background = "#000000",
    },
    actions = {},
}

return config
LUA
    echo "Wrote a blank init.lua"
    ;;

gore)
    TMP="$(mktemp -d)"
    trap 'rm -rf "$TMP"' EXIT

    echo "Downloading gore's generic config..."
    git clone --depth 1 -q "$GORE_URL" "$TMP/gore"

    # Move, never overwrite. Someone running this has already lost one config
    # to a script that did not ask.
    for path in "$TMP"/gore/*; do
        name="$(basename "$path")"
        case "$name" in
            .git|LICENSE|README.md) continue ;;
            resources) continue ;;  # merged below, not replaced
        esac

        if [ -e "$CONFIG_DIR/$name" ]; then
            mv "$CONFIG_DIR/$name" "$CONFIG_DIR/$name.before-gore"
            echo "Moved $name -> $name.before-gore"
        fi
        cp -r "$path" "$CONFIG_DIR/$name"
    done

    # Its images and jars go alongside whatever is already in resources.
    mkdir -p "$CONFIG_DIR/resources"
    cp -rn "$TMP"/gore/resources/. "$CONFIG_DIR/resources/" 2>/dev/null || true

    echo "Installed gore's generic config"
    echo "Its settings live at the top of $CONFIG_DIR/init.lua"
    ;;
esac

# ---------------------------------------------------------------------------
# And take toolwall out.
# ---------------------------------------------------------------------------

rm -f "$CONFIG_DIR/toolwall.lua"
rm -rf "$CONFIG_DIR/toolwall"
echo "Removed the runtime"

DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
if [ -f "$DATA/applications/toolwall-setup.desktop" ]; then
    rm -f "$DATA/applications/toolwall-setup.desktop"
    rm -rf "$DATA/toolwall"
    command -v update-desktop-database >/dev/null 2>&1 &&
        update-desktop-database "$DATA/applications" 2>/dev/null || true
    echo "Removed the app menu entry"
fi

if [ -n "$ALL" ]; then
    rm -f "$CONFIG_DIR/toolwall.json" "$CONFIG_DIR/toolwall-import-report.txt"
    echo "Removed toolwall.json and the import report"
else
    echo "Kept $CONFIG_DIR/toolwall.json"
fi

echo
echo "The editor and CLI are cargo's, not ours:"
echo "  cargo uninstall toolwall-gui toolwall-cli"
echo
echo "toolwall also leaves a log at ~/.local/state/toolwall.log"
