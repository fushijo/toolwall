#!/usr/bin/env sh
# Install the toolwall Lua runtime into the waywall config directory.
# The CLI and GUI are installed separately with cargo.
set -eu

CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/waywall"
SRC="$(cd "$(dirname "$0")" && pwd)"

if [ ! -d "$CONFIG_DIR" ]; then
    echo "waywall config directory not found: $CONFIG_DIR" >&2
    echo "Run waywall once so it generates one, then re-run this script." >&2
    exit 1
fi

echo "Installing runtime -> $CONFIG_DIR"
rm -rf "$CONFIG_DIR/toolwall"
cp -r "$SRC/runtime/toolwall" "$CONFIG_DIR/toolwall"
cp "$SRC/runtime/toolwall.lua" "$CONFIG_DIR/toolwall.lua"

if [ -f "$CONFIG_DIR/init.lua" ] && [ ! -f "$CONFIG_DIR/init.lua.pre-toolwall" ]; then
    echo "Backing up init.lua -> init.lua.pre-toolwall"
    cp "$CONFIG_DIR/init.lua" "$CONFIG_DIR/init.lua.pre-toolwall"
fi

cat > "$CONFIG_DIR/init.lua" <<'LUA'
local toolwall = require("toolwall")
return toolwall.setup()
LUA

echo "Installing overlays -> $CONFIG_DIR/resources"
mkdir -p "$CONFIG_DIR/resources"
cp -n "$SRC/resources/measuring_overlay.png" "$CONFIG_DIR/resources/" 2>/dev/null || true

if [ ! -f "$CONFIG_DIR/toolwall.json" ]; then
    echo "Installing starter config -> $CONFIG_DIR/toolwall.json"
    cp "$SRC/examples/default.json" "$CONFIG_DIR/toolwall.json"
else
    echo "Keeping existing toolwall.json"
fi

echo
echo "Done. Next:"
echo "  cargo install --path crates/toolwall-cli"
echo "  cargo install --path crates/toolwall-gui"
echo "  toolwall validate"
echo
echo "Optional, for the Ninjabrain Bot readout's panel and text outline:"
echo "  patches/apply.sh ~/waywall"
echo "Stock waywall has no way to fill a rectangle. See patches/README.md."

echo
echo "waywall exec()s the GUI with its own PATH, which usually does not"
echo "include ~/.cargo/bin. gui.command is set to ~/.cargo/bin/toolwall-gui"
echo "for that reason - keep it absolute or ~-prefixed."
