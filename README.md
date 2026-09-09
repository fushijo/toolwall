# toolwall

A GUI and configuration layer for [waywall](https://github.com/tesselslate/waywall),
the Wayland compositor for Minecraft speedrunning on Linux.

waywall is configured by hand-writing Lua. toolwall makes that configuration
**data** — a single JSON document — and gives you a CLI and an in-game GUI to
edit it, with changes applying live.

> Status: early. The Lua runtime is implemented and tested; the GUI is a
> skeleton. See [Milestones](#milestones).

## Why not just fork waywall

Because a fork is a worse product. toolwall is deliberately *not* a patched
waywall:

- It installs alongside any waywall version and survives waywall updates.
- People can adopt the runtime library without the GUI, or the GUI without your
  presets.
- It stays licensable separately (see [Licence](#licence)).

The entire integration surface is two facts about waywall:

1. Its Lua config can be a thin shim that reads a data file.
2. It hot-reloads on any `.lua` change in its config directory.

That is enough for live-applying config edits with **zero changes to waywall's
C source**.

## How it works

```
  toolwall-gui  ─┐
                 ├─→  toolwall.json  ─→  toolwall.lua runtime  ─→  waywall
  toolwall CLI  ─┘         ▲                    (scene objects,
                           │                     modes, keybinds)
                  toolwall_reload.lua
                  (bumped to trip the watcher)
```

The GUI and CLI never talk to waywall. They write JSON, then bump a counter in
`toolwall_reload.lua`. waywall's file watcher sees the `.lua` change, rebuilds
its Lua VM, and the runtime re-reads the JSON.

**This trigger file is load-bearing.** waywall watches `.lua` files only — it
will never notice a change to `toolwall.json` on its own.

## Install

Requires a working waywall setup, a Rust toolchain, and `luajit` if you want to
run the runtime tests.

```sh
git clone https://github.com/OWNER/toolwall
cd toolwall

./install.sh                              # runtime + overlays into ~/.config/waywall
cargo install --path crates/toolwall-cli  # the `toolwall` command
cargo install --path crates/toolwall-gui  # the in-game editor

toolwall validate
```

`install.sh` copies the Lua runtime and the measuring overlay into your waywall
config directory, backs up your existing `init.lua` as `init.lua.pre-toolwall`,
and replaces it with two lines:

```lua
local toolwall = require("toolwall")
return toolwall.setup()
```

waywall adds its config directory to `package.path`, which is what makes
`require("toolwall")` resolve.

### Two things that will otherwise catch you out

**`gui.command` needs a full path.** waywall `exec()`s it with the compositor's
own `PATH`, which comes from however waywall was launched - a launcher, a
desktop entry - and so usually does *not* include `~/.cargo/bin` the way an
interactive shell does. A bare `toolwall-gui` fails silently with nothing but
an `execvp` line in waywall's log. The shipped config uses
`~/.cargo/bin/toolwall-gui`.

**Overlay coordinates are waywall window pixels, not your monitor.** waywall
reports its window size to the instance - check `Display:` in F3. On a 1920x1200
panel at 125% scaling that is 1707x1067, and an overlay positioned past that
edge simply does not draw. No warning, no error.

## Use

```sh
toolwall validate                                  # check without applying
toolwall modes                                     # list modes
toolwall get modes.thin.resolution.width
toolwall set modes.thin.resolution.width 340       # applies live
toolwall reload                                    # re-trigger without editing
```

Dotted paths index arrays by their `id`, so `modes.thin` works and survives
reordering.

In game, `Ctrl+I` opens the editor as a floating window. Edits apply about a
third of a second after you stop making them - there is no save button. The
mode you are in survives the reload, so you can nudge a rectangle and watch it
move.

Minecraft has to let go of the cursor before you can click a floating window;
opening the editor presses Escape into the game to do that. `Esc` closes the
editor.

## Measuring window

The boat-eye measuring view is a mirror magnifying a slice of the game, with
the numbered grid drawn on the *same rectangle* so its centre line lands on the
crosshair. Two numbers decide whether it measures anything real:

- The capture must be **60 game pixels wide**. The grid is 18 bands, so 60/18
  makes one band exactly one game pixel.
- The destination width must be a **multiple of 60**, or a band is a
  fractional number of screen pixels and the error compounds across the scale.
  960, 720 and 600 are exact; 980 is not, and drifts about two pixels end to
  end.

The capture is anchored `center`, so it tracks the crosshair as the resolution
changes rather than needing to be re-placed per mode.



```sh
toolwall validate                                  # check without applying
toolwall modes                                     # list modes
toolwall get modes.thin.resolution.width
toolwall set modes.thin.resolution.width 340       # applies live
toolwall reload                                    # re-trigger without editing
```

Dotted paths index arrays by their `id`, so `modes.thin` works and survives
reordering.

In game, `Ctrl+I` opens the GUI as a floating window.

## Config model

| Concept | What it is |
| --- | --- |
| **mode** | A resolution + optional sensitivity + the set of overlays live while selected. Thin BT, wide, eye measure. |
| **mirror** | A region of the Minecraft window drawn elsewhere, with optional colour keying and shader. |
| **image** | A PNG overlay. |
| **text** | A HUD element with `{mode}`, `{res}`, `{sens}`, `{state}` placeholders. |
| **keybind** | An input string bound to a command from a closed set. |

Keybinds name commands; they never contain Lua source. A config downloaded from
someone else cannot execute arbitrary code when it loads. `exec` exists but is
off unless `gui.allow_exec` is set.

`schema/toolwall.schema.json` is the contract. Point your editor at it for
completion and inline validation.

## Testing

```sh
luajit tests/run.lua        # runtime, against a mock waywall
cargo test --workspace      # schema, validation, and headless GUI layout
```

`tests/mock/waywall.lua` stands in for the real module and enforces the two
lifecycle rules that are painful to debug live: most API calls are illegal
during startup, and a closed scene object may not be reused.

You can develop the entire runtime without launching Minecraft.

## Milestones

- [x] **M0** — Schema extracted from a working hand-written config
- [x] **M1** — Lua runtime; behaviour identical to a hand-written config
- [x] **M2** — CLI proving the write path and hot reload
- [ ] **M3** — GUI (Modes tab wired; the rest follow the same pattern)
- [ ] **M4** — HUD polish and per-mode text
- [ ] **M5** — Upstream: per-window floating control and generalised anchoring

## Known limitations

These are waywall constraints, not bugs here:

- **`show_floating` is global.** Opening the GUI also reveals Ninjabrain Bot.
  Fixing this properly means an upstream patch for per-window control.
- **Anchoring is Ninjabrain-only.** `theme.ninb_anchor` is hardcoded; other
  floating windows can only be shift-dragged.
- **No mouse position in Lua.** waywall exposes key and button actions and
  `get_key()` polling, but no cursor coordinates. This is why the GUI is a
  floating window rather than drawn with scene objects.
- **Scene objects have no visibility flag.** Hiding means closing and
  recreating. Negative depth is not a substitute — it draws *behind* Minecraft,
  which only hides things while Minecraft covers that region.
- **Ninjabrain Bot calibration does not work correctly inside waywall.** Use
  boat eye, or calibrate outside.

## Licence

MIT.

toolwall is a separate program that communicates with waywall through
configuration files. It does not link against or derive from waywall's source,
so waywall's **GPL-3.0-only** licence does not extend to it.

If you patch or vendor waywall's C source into this repository, that changes:
the derivative work must then be GPL-3.0-only. Keep upstream patches in a
separate repository or as PRs against waywall itself.

## Credits

- [waywall](https://github.com/tesselslate/waywall) by tesselslate — the
  compositor this is built on.
- [Toolscreen](https://github.com/jojoe77777/Toolscreen) by jojoe77777 — the
  Windows tool whose feature set defined the target. No shared code; toolwall
  is not affiliated with or endorsed by it.
- [Linux MCSR Resources](https://linux-mcsr-resources.github.io/) — the
  community index.
- The measuring overlay in `resources/` is from the waywall generic config by
  Arjun Gore, used under the MIT Licence. Its capture geometry, the pie chart
  source rectangles and the god-sens multipliers all come from that config.
