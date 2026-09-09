# Architecture

This document records *why* toolwall is shaped the way it is. Read it before
changing the integration boundary.

## The constraint that decides everything

waywall's Lua API exposes:

- `actions`, key/button combos with modifiers, plus `get_key()` polling
- `mirror()`, copy a rect of the Minecraft window elsewhere
- `image()`, load a PNG **from a filesystem path**
- `text()`, draw a string in a bundled bitmap font
- custom GLSL shaders on any of the above

It does **not** expose the mouse cursor position. There is no motion event, no
hover, no coordinate query.

It also has no dynamic drawing primitive. No rectangle, no line, no pixel
buffer. `image()` reads a file off disk.

So a settings UI drawn with waywall scene objects would be keyboard-only, with
every panel pre-rendered as a PNG. That is a fine way to build a BIOS menu and a
bad way to build something with sliders and rect pickers.

**Therefore the GUI is a separate Wayland client**, launched with
`waywall.exec()` and hosted as a floating window, exactly what Ninjabrain Bot
already is. That path is proven in production by every waywall user running
ninb.

## The second constraint, which makes it cheap

waywall watches its config directory and hot-reloads on any `.lua` change,
rebuilding the Lua VM.

So the GUI needs no IPC, no socket, no patched waywall. It writes a file. The
runtime re-reads it. Live apply, for free.

The catch: **waywall watches `.lua` files, not `.json`.** Writing
`toolwall.json` alone does nothing. Every save must also rewrite
`toolwall_reload.lua` with changed content, a monotonic counter, since some
watchers coalesce byte-identical writes. See `store::Store::trigger_reload`.

Order matters: JSON is written atomically via temp-file-plus-rename *first*, so
the watcher can never observe a half-written document, and the trigger is
written last.

## Why data-driven, and why the runtime came first

The original config was hand-written Lua. A GUI cannot safely edit hand-written
Lua, you would be parsing and rewriting arbitrary code, destroying comments and
control flow, and the round-trip would never be lossless.

So the first deliverable was not the GUI. It was separating declarative data
from imperative config:

1. **Schema.** Enumerate what actually varies.
2. **Runtime.** A Lua library that reads the data and produces identical
   behaviour to the hand-written config. Checkpoint: nothing changes for the
   user.
3. **CLI.** Prove the write path and hot reload with no pixels involved.
4. **GUI.** Now it is just an editor for a JSON file, a solved problem.

Building the GUI first would have meant iterating the data model through the
UI, which is the expensive direction.

## Lifecycle: the trap that will bite you

waywall forbids most API calls "during startup". `init.lua` must return its
config table synchronously, but `set_resolution`, `mirror`, `image`, `text`,
`exec` and friends all throw if called at that point.

`toolwall.setup()` therefore does two things:

- builds and returns the config table (input, theme, window, shaders, actions)
- registers a `load` listener, and does *all* scene work inside it

Keybind handlers are built during startup but only *run* later, so they close
over runtime state that does not exist yet and guard on `rt.modes` being
populated.

`tests/mock/waywall.lua` enforces this rule, so violations fail in CI instead of
in a run.

## Garbage collection

waywall scene objects disappear when the Lua VM garbage-collects them. Anything
created and then dropped will silently vanish from the screen.

The runtime holds every live object in `Scene.live`, which is reachable from the
module-level `rt` table for the lifetime of the VM. Do not restructure this into
locals.

## Hiding things

Scene objects have `close()`, `get_depth()` and `set_depth()`. There is no
visibility flag.

Negative depth places an object *behind* the Minecraft instance, which is only
equivalent to hidden while Minecraft covers that region. At thin or tall
resolutions it does not, and the object reappears over the background.

So hiding is close-and-recreate. Mirrors are cheap. Images re-read the PNG,
which is acceptable at mode-switch frequency but would not be in a loop.

## Failure handling

A GUI that writes config which hot-reloads means a bad write can break a live
session mid-run. Two defences:

- The CLI and GUI round-trip through the typed model and validate before
  writing. An invalid document is never persisted.
- The runtime snapshots each successful load as `toolwall.json.last-good`. If a
  load fails, it falls back to that snapshot and draws a banner rather than
  throwing, so the session survives.

Individual scene objects fail soft too: a missing PNG logs a warning and skips
that overlay instead of taking down the config.

## Security posture

Keybinds name commands from a closed enum. They never carry Lua source.

This matters because configs get shared, the Linux MCSR Resources site has a
whole category for them, and waywall's own documentation warns that config Lua
can spawn subprocesses and write files. A toolwall config downloaded from a
stranger cannot execute arbitrary code on load. The `exec` command exists but is
gated behind `gui.allow_exec`, which defaults to false.

## Open upstream work

`waywall.show_floating()` is global, it shows and hides every floating window
at once. Anchoring via `theme.ninb_anchor` is hardcoded to Ninjabrain Bot.

Consequence: opening the toolwall GUI also reveals ninb, and the GUI window
cannot be anchored, only shift-dragged.

The fix is upstream: per-window floating control and generalised anchoring. That
patch belongs in waywall's repository under GPL-3.0-only, not vendored here.
