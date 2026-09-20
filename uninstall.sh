#!/usr/bin/env sh
# Take toolwall back out of the waywall config directory.
#
# Restores the init.lua that was there before, removes the runtime, and leaves
# your toolwall.json alone so a reinstall picks up where you left off.
#
#   --all   also delete toolwall.json and the import report
#   -y      do not ask
set -eu

CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/waywall"
BACKUP="$CONFIG_DIR/init.lua.pre-toolwall"

ALL=""
YES=""
for arg in "$@"; do
    case "$arg" in
        --all) ALL=1 ;;
        -y|--yes) YES=1 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

if [ ! -d "$CONFIG_DIR" ]; then
    echo "no waywall config directory at $CONFIG_DIR, nothing to do"
    exit 0
fi

echo "This will remove from $CONFIG_DIR:"
echo "  toolwall.lua"
echo "  toolwall/"
if [ -f "$BACKUP" ]; then
    echo "and put init.lua.pre-toolwall back as init.lua."
else
    echo "init.lua will be left as the toolwall shim: there is no backup to"
    echo "restore, so edit it yourself or let waywall write a fresh one."
fi
if [ -n "$ALL" ]; then
    echo "--all: toolwall.json and toolwall-import-report.txt go too."
else
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

# Restore first. If this is going to fail it should fail before the runtime
# that init.lua needs has been deleted out from under it.
if [ -f "$BACKUP" ]; then
    # Writing to a symlink writes through it, the same way installing did. If
    # init.lua points into a dotfiles repo, that is where the original lands.
    if [ -L "$CONFIG_DIR/init.lua" ]; then
        echo "Note: init.lua is a symlink to $(readlink "$CONFIG_DIR/init.lua")"
        echo "      Restoring writes through it, into that file."
    fi

    cp "$BACKUP" "$CONFIG_DIR/init.lua"
    echo "Restored init.lua from init.lua.pre-toolwall"

    # The copy has done its job, and leaving it behind is not harmless: a
    # later install.sh only takes a backup when there is not one already, so a
    # stale one here means the next real config never gets saved.
    rm -f "$BACKUP"
else
    echo "No init.lua.pre-toolwall, so init.lua is left alone."
fi

rm -f "$CONFIG_DIR/toolwall.lua"
rm -rf "$CONFIG_DIR/toolwall"
echo "Removed the runtime"

if [ -n "$ALL" ]; then
    rm -f "$CONFIG_DIR/toolwall.json" "$CONFIG_DIR/toolwall-import-report.txt"
    echo "Removed toolwall.json and the import report"
else
    echo "Kept $CONFIG_DIR/toolwall.json"
fi

echo
echo "The editor and CLI are cargo's, not ours:"
echo "  cargo uninstall toolwall-gui toolwall-cli"
