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

# waywall never makes this directory. It watches it and reads init.lua out of
# it, and that is all, so somebody who installed waywall five minutes ago does
# not have one. This used to stop here and tell them to "run waywall once so it
# generates one", which is advice that does not work.
if [ ! -d "$CONFIG_DIR" ]; then
    echo "Creating $CONFIG_DIR"
    mkdir -p "$CONFIG_DIR"
fi

if ! command -v waywall >/dev/null 2>&1; then
    echo
    echo "Note: waywall is not on your PATH. toolwall configures it, it does"
    echo "not install it. If you have not got it yet, start here:"
    echo "  https://github.com/tesselslate/waywall"
    echo
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

# The importer goes in too, so the setup window can convert a config later on
# without the tarball still being around. It finds keycodes.lua beside itself.
cp "$SRC/tools/import.lua" "$CONFIG_DIR/toolwall/import.lua"

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
# surprise worth naming. it is not a backup you should rely on.
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
echo "Runtime installed."

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

# ---------------------------------------------------------------------------
# The editor, and the window that sets it up
#
# Both are cargo's, and this script runs before either exists, which is why
# the setup window cannot simply be the installer. Offer to build them, then
# hand over: a first setup is a conversation, and a shell script is bad at
# those.
# ---------------------------------------------------------------------------

ask() {
    [ -t 0 ] || return 1
    printf "%s [Y/n] " "$1"
    read -r reply
    case "$reply" in
        n|N|no|NO) return 1 ;;
        *) return 0 ;;
    esac
}

find_gui() {
    if [ -x "$HOME/.cargo/bin/toolwall-gui" ]; then
        echo "$HOME/.cargo/bin/toolwall-gui"
    elif command -v toolwall-gui >/dev/null 2>&1; then
        command -v toolwall-gui
    fi
}

GUI="$(find_gui)"

echo
if [ -z "$GUI" ] && [ -z "${TOOLWALL_NO_BUILD:-}" ] && command -v cargo >/dev/null 2>&1; then
    if ask "Build the editor and CLI now? Takes a few minutes the first time."; then
        cargo install --path "$SRC/crates/toolwall-cli"
        cargo install --path "$SRC/crates/toolwall-gui"
        GUI="$(find_gui)"
    fi
fi

if [ -n "$GUI" ]; then
    if ask "Open the setup window?"; then
        # Not fatal. A machine with no display still has a working install,
        # and saying so beats the script dying on its last line.
        "$GUI" --setup || echo "The setup window did not open. Run: $GUI --setup"
    else
        echo "Run it later with: $GUI --setup"
    fi
else
    echo "Still to do:"
    echo "  cargo install --path crates/toolwall-cli"
    echo "  cargo install --path crates/toolwall-gui"
    echo "  toolwall-gui --setup"
fi

# ---------------------------------------------------------------------------
# A desktop entry, so setup is in the app menu and not only in a terminal
#
# Icon by absolute path on purpose. The themed icon directories want exact
# pixel sizes in the folder name, and a 300x300 logo fits none of them.
# ---------------------------------------------------------------------------

DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
ICON="$DATA/toolwall/toolwall.png"

if [ -n "$GUI" ]; then
    mkdir -p "$DATA/toolwall" "$DATA/applications"
    cp "$SRC/resources/toolwall.png" "$ICON"

    # One entry per window, because Wayland picks a window's icon by matching
    # its app id against a .desktop file's name. Only the setup one is meant
    # to be clicked; the other two are NoDisplay and exist purely so the
    # editor and your instance stop showing up as a grey placeholder.
    #
    # waywall.desktop is a file we install for somebody else's window. It
    # ships no entry of its own, so nothing is being overridden, and
    # uninstall.sh takes it back out. Set TOOLWALL_NO_WAYWALL_ICON=1 to skip.
    entries="toolwall-setup.desktop toolwall.desktop"
    [ -z "${TOOLWALL_NO_WAYWALL_ICON:-}" ] && entries="$entries waywall.desktop"

    for entry in $entries; do
        sed -e "s|@EXEC@|$GUI|" -e "s|@ICON@|$ICON|" \
            "$SRC/resources/$entry" > "$DATA/applications/$entry"
    done

    # Some desktops only notice a new entry once this has run, and plenty of
    # systems do not ship it. Neither case is worth failing over.
    command -v update-desktop-database >/dev/null 2>&1 &&
        update-desktop-database "$DATA/applications" 2>/dev/null || true

    echo "Added \"toolwall setup\" to your app menu."
fi

echo
echo "Ctrl-I opens the editor once you are in game."

echo
echo "Optional, for the Ninjabrain Bot readout's panel and text outline:"
echo "  patches/apply.sh ~/waywall"
echo "Stock waywall has no way to fill a rectangle. See patches/README.md."
echo "Leave this alone on a first setup."

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
