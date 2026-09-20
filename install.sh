#!/usr/bin/env sh
# Install the toolwall Lua runtime into the waywall config directory.
# The CLI and GUI are installed separately with cargo.
#
# If you already have a waywall config, this imports it: your resolutions,
# mirrors, overlays, rebinds and keybinds become the starting toolwall.json,
# and anything that could not be carried across is listed at the end rather
# than dropped quietly. Your init.lua is backed up either way.
#
#   TOOLWALL_NO_IMPORT=1   skip the import and install the starter config
set -eu

CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/waywall"
SRC="$(cd "$(dirname "$0")" && pwd)"

if [ ! -d "$CONFIG_DIR" ]; then
    echo "waywall config directory not found: $CONFIG_DIR" >&2
    echo "Run waywall once so it generates one, then re-run this script." >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Import, before anything is overwritten.
#
# This has to happen first. The import reads the config by running it, and
# three lines further down init.lua stops being that config.
# ---------------------------------------------------------------------------

find_lua() {
    for exe in luajit lua5.1 lua; do
        if command -v "$exe" >/dev/null 2>&1; then
            echo "$exe"
            return 0
        fi
    done
    return 1
}

IMPORTED=""

if [ -f "$CONFIG_DIR/toolwall.json" ]; then
    # Worth spelling out. Someone reinstalling to pick up an import fix will
    # sit here watching it not happen, because the file it would write is
    # already there.
    echo "Keeping existing toolwall.json, so nothing is imported."
    echo "To import your waywall config again, move that file out of the way"
    echo "first, or run ./uninstall.sh --all and then install."
elif [ ! -f "$CONFIG_DIR/init.lua" ]; then
    : # nothing to import from
elif grep -q 'require("toolwall")' "$CONFIG_DIR/init.lua" 2>/dev/null; then
    : # already a toolwall shim, so there is no waywall config left to read
elif [ -n "${TOOLWALL_NO_IMPORT:-}" ]; then
    echo "Skipping import (TOOLWALL_NO_IMPORT is set)"
elif LUA="$(find_lua)"; then
    echo "Importing your waywall config..."
    echo

    STAGED="$CONFIG_DIR/.toolwall.json.import"
    if "$LUA" "$SRC/tools/import.lua" "$CONFIG_DIR" "$STAGED"; then
        IMPORTED="$STAGED"
    else
        echo
        echo "Could not import it. Falling back to the starter config; your" >&2
        echo "own config is untouched and still backed up below." >&2
        rm -f "$STAGED"
    fi
    echo
else
    echo "No lua interpreter found, so your config cannot be imported."
    echo "Install luajit and re-run to import it instead of starting fresh."
    echo
fi

# ---------------------------------------------------------------------------
# Runtime
# ---------------------------------------------------------------------------

echo "Installing runtime -> $CONFIG_DIR"
rm -rf "$CONFIG_DIR/toolwall"
cp -r "$SRC/runtime/toolwall" "$CONFIG_DIR/toolwall"
cp "$SRC/runtime/toolwall.lua" "$CONFIG_DIR/toolwall.lua"

# Back up the config being replaced, once.
#
# Not on a re-install, though: by then init.lua is already our own two-line
# shim, and saving that would fill the one backup slot with a copy of the thing
# doing the replacing. The guard below is `! -f`, so a worthless backup here
# means a real one can never be taken.
if [ -f "$CONFIG_DIR/init.lua" ] &&
   [ ! -f "$CONFIG_DIR/init.lua.pre-toolwall" ] &&
   ! grep -q 'require("toolwall")' "$CONFIG_DIR/init.lua" 2>/dev/null; then
    echo "Backing up init.lua -> init.lua.pre-toolwall"
    cp "$CONFIG_DIR/init.lua" "$CONFIG_DIR/init.lua.pre-toolwall"
fi

# Writing to a symlink writes through it. If init.lua points into a dotfiles
# repo, the shim lands in the repo and shows up as a change there, which is a
# surprise worth naming rather than a backup worth relying on.
if [ -L "$CONFIG_DIR/init.lua" ]; then
    echo "Note: init.lua is a symlink to $(readlink "$CONFIG_DIR/init.lua")"
    echo "      That file is what gets rewritten. The copy above is your backup."
fi

cat > "$CONFIG_DIR/init.lua" <<'LUA'
local toolwall = require("toolwall")
return toolwall.setup()
LUA

echo "Installing overlays -> $CONFIG_DIR/resources"
mkdir -p "$CONFIG_DIR/resources"
cp -n "$SRC/resources/measuring_overlay.png" "$CONFIG_DIR/resources/" 2>/dev/null || true

if [ -n "$IMPORTED" ]; then
    mv "$IMPORTED" "$CONFIG_DIR/toolwall.json"
    echo "Installing imported config -> $CONFIG_DIR/toolwall.json"
elif [ ! -f "$CONFIG_DIR/toolwall.json" ]; then
    echo "Installing starter config -> $CONFIG_DIR/toolwall.json"
    cp "$SRC/examples/default.json" "$CONFIG_DIR/toolwall.json"
fi

echo
echo "Done. Next:"
echo "  cargo install --path crates/toolwall-cli"
echo "  cargo install --path crates/toolwall-gui"
echo "  toolwall validate"
echo
echo "Then launch your instance. Ctrl-I opens the editor."

if [ -n "$IMPORTED" ]; then
    echo
    echo "Everything the import could not carry across is written down in:"
    echo "  $CONFIG_DIR/toolwall-import-report.txt"
    echo "Read it. It also names the key it gave the editor, which is not"
    echo "Ctrl-I if your own config had already taken that."
fi

if [ -f "$CONFIG_DIR/init.lua.pre-toolwall" ]; then
    echo
    echo "Your old config is untouched. init.lua is now a two-line shim, and the"
    echo "files it used to load (main.lua, remaps.lua, ...) are still sitting"
    echo "there unread. To undo all of this and get init.lua back:"
    echo "  ./uninstall.sh"
fi

echo
echo "Optional, for the Ninjabrain Bot readout's panel and text outline:"
echo "  patches/apply.sh ~/waywall"
echo "Stock waywall has no way to fill a rectangle. See patches/README.md."

echo
if ! command -v toolwall >/dev/null 2>&1; then
    echo "Note: cargo installs into ~/.cargo/bin, which is not on your PATH."
    echo "An interactive shell reads ~/.bashrc, not ~/.profile, so add it there:"
    echo "  echo 'PATH=\"\$HOME/.cargo/bin:\$PATH\"' >> ~/.bashrc"
    echo
fi

echo "waywall exec()s the GUI with its own PATH, which usually does not"
echo "include ~/.cargo/bin. gui.command is set to ~/.cargo/bin/toolwall-gui"
echo "for that reason - keep it absolute or ~-prefixed."
