# toolwall

A GUI and configuration layer for [waywall](https://github.com/tesselslate/waywall),
the Wayland compositor for Minecraft speedrunning on Linux.

> Inspired by [Toolscreen](https://github.com/jojoe77777/Toolscreen), which is
> Windows only. There was no equivalent on Linux, so this is an attempt at one.
> Plenty more is planned over the coming days.
>
> I used Claude to help debug this and to write the code comments.

waywall is configured by hand-writing Lua. toolwall turns that configuration
into data, a single JSON file, and gives you a CLI and an in-game editor to
change it, with edits applying live.

## Why not fork waywall

toolwall is not a patched waywall, on purpose:

- It installs alongside any waywall version and survives waywall updates.
- You can use the runtime library without the GUI, or the GUI without the
  presets.
- It stays separately licensable (see [Licence](#licence)).

The whole integration relies on two facts about waywall:

1. Its Lua config can be a small shim that reads a data file.
2. It hot-reloads on any `.lua` change in its config directory.

That is enough to apply config edits live with no changes to waywall's C
source.

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

That trigger file is doing real work. waywall watches `.lua` files only, so it
will never notice a change to `toolwall.json` by itself.

## Screenshots

### Thin BT with the overlays live

![Thin BT at 340x1080 with entity counter, mirrored pie chart and magnified percentages](docs/screenshots/01-thin-bt-overlays.png)

Thin BT mode at 340x1080. The game is the narrow strip down the middle; the
rest of the screen is background image. Everything else here is drawn by
toolwall:

- **Top left**: the HUD text, showing the active mode and its resolution.
- **`0/49`**: the entity counter, a mirror of the `E:` line from F3, captured
  at 37x9 game pixels and drawn 5x larger so it is readable at a glance while
  eraying a bastion.
- **Right**: the F3 pie chart, mirrored out of the bottom-right corner of the
  game and magnified. It is drawn as one layer per pie colour, because colour
  keying passes only the colour it matches, which is what lifts the chart off
  the world behind it.
- **`7.46%` / `2.89%`**: the pie percentages as their own overlay, captured
  from a 33x25 region and drawn at 6x.

Captures are anchored to a corner rather than pinned to absolute coordinates,
so the same overlay stays correct when the resolution changes.

### The editor: Modes

![The Modes tab with Thin BT expanded](docs/screenshots/02-editor-modes.png)

`Ctrl+I` opens the editor over the game. Modes set a resolution, an optional
sensitivity override, and which overlays come up automatically. Basic hides the
ids and layering; Advanced shows everything.

Note the status bar: there is no save button. Edits apply about a third of a
second after you stop making them, and the mode you are in survives the reload.

### The editor: Mirrors

![The Mirrors tab with the entity counter expanded](docs/screenshots/03-editor-mirrors.png)

Every rectangle has -/+ steppers, because placing an overlay is nudging rather
than typing. Changes apply live, so the overlay moves on screen while you
adjust it.

### The editor: Theme

![The Theme tab, with Ninjabrain Bot open beside it](docs/screenshots/04-editor-theme.png)

Background colour picks from a swatch, background image has a file browser.
Ninjabrain Bot is open on the right, launched by toolwall and positioned by the
anchor settings shown here.

The editor styles itself from the same config: opacity over the game, dark or
light, and font size, all applied as you drag them.

## Install

You need a working waywall setup, a Rust toolchain, and `luajit` if you want to
run the runtime tests.

```sh
git clone https://github.com/fushijo/toolwall
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

### Two things that will otherwise waste your time

**`gui.command` needs a full path.** waywall `exec()`s it using the
compositor's own `PATH`, which comes from however waywall was launched, so it
usually does not include `~/.cargo/bin` the way an interactive shell does. A
bare `toolwall-gui` fails silently, leaving only an `execvp` line in waywall's
log. The shipped config uses `~/.cargo/bin/toolwall-gui`.

**Overlay coordinates are waywall window pixels, not your monitor.** waywall
reports its window size to the instance, which you can read from `Display:` in
F3. On a 1920x1200 panel at 125% scaling that is 1707x1067. An overlay
positioned past that edge just does not draw, with no warning and no error.

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
third of a second after you stop making them, so there is no save button. The
mode you are in survives the reload, which means you can nudge a rectangle and
watch it move. `Esc` closes the editor.

Minecraft has to release the cursor before you can click a floating window.
Opening the editor presses Escape into the game to make that happen.

## Measuring window

The boat-eye measuring view is a mirror magnifying a slice of the game, with
the numbered grid drawn on the same rectangle, so its centre line lands on the
crosshair. Two numbers decide whether it measures anything real:

- The capture has to be **60 game pixels wide**. The grid is 18 bands, and
  60/18 makes one band exactly one game pixel.
- The destination width has to be a **multiple of 60**. Otherwise a band covers
  a fractional number of screen pixels and the error adds up across the scale.
  960, 720 and 600 are exact. 980 is not, and drifts about two pixels end to
  end.

The capture is anchored `center`, so it follows the crosshair as the resolution
changes instead of needing to be repositioned for each mode.

## Config model

| Concept | What it is |
| --- | --- |
| **mode** | A resolution, an optional sensitivity, and the overlays live while it is selected. Thin BT, wide, eye measure. |
| **mirror** | A region of the Minecraft window drawn elsewhere, with optional colour keying and shader. |
| **image** | A PNG overlay. |
| **text** | A HUD element with `{mode}`, `{res}`, `{sens}`, `{state}` placeholders. |
| **keybind** | An input string bound to a command from a fixed set. |

Captures can be anchored to a corner or to the centre, so one rectangle stays
correct at every resolution. Minecraft pins its debug HUD to the corners at
fixed pixel offsets, which is why a capture written in absolute coordinates
only works at the resolution it was written for.

A mirror can carry several colour keys, drawn as one layer each. Colour keying
passes only the matching colour, so pulling something many-coloured like the
pie chart out of the world behind it means stacking one layer per colour. A
crop shader does the same job in one pass and looks better, but waywall
compiles shaders at startup only, so shaders cannot be edited live.

Keybinds name commands and never contain Lua source, so a config downloaded
from someone else cannot run arbitrary code when it loads. `exec` exists but is
off unless `gui.allow_exec` is set.

`schema/toolwall.schema.json` is the contract. Point your editor at it for
completion and inline validation.

## Testing

```sh
luajit tests/run.lua        # runtime, against a mock waywall
cargo test --workspace      # schema, validation, and headless GUI layout
```

`tests/mock/waywall.lua` stands in for the real module and enforces the
lifecycle rules that are painful to debug live: most API calls are illegal
during startup, a closed scene object cannot be reused, and `set_resolution`
fails until the Minecraft window exists.

The GUI tabs are covered by headless egui layout passes, so no compositor or
GPU is needed. You can develop the whole thing without launching Minecraft.

## Status

Working: modes, mirrors, images, keybinds, input, the measuring window, pie
chart and entity counter overlays, Ninjabrain Bot launching, and the in-game
editor with live apply.

Planned:

- More of Toolscreen's feature set.
- A visual rectangle editor, so overlays can be dragged instead of typed.
- HUD text polish and per-mode text.
- Upstream waywall work: per-window floating control and general anchoring.

## Known limitations

These come from waywall, not from toolwall:

- **`show_floating` is global.** Opening the editor also reveals Ninjabrain
  Bot. Fixing it properly needs an upstream patch for per-window control.
- **There is one anchor slot**, and it goes to whichever floating window opened
  first. Other floating windows can only be shift-dragged.
- **No mouse position in Lua.** waywall exposes key and button actions and
  `get_key()` polling, but no cursor coordinates. That is why the editor is a
  floating window rather than drawn with scene objects.
- **Scene objects have no visibility flag.** Hiding means closing and
  recreating. Negative depth is not a substitute, since it draws behind
  Minecraft, which only hides things while Minecraft covers that region.
- **Shaders compile at startup only**, so they cannot be edited live.
- **Ninjabrain Bot calibration does not work correctly inside waywall.** Use
  boat eye, or calibrate outside.

## Licence

MIT.

toolwall is a separate program that talks to waywall through configuration
files. It does not link against or derive from waywall's source, so waywall's
GPL-3.0-only licence does not extend to it.

If you patch or vendor waywall's C source into this repository that changes,
and the derivative work has to be GPL-3.0-only. Keep upstream patches in a
separate repository or as PRs against waywall itself.

## Credits

- [waywall](https://github.com/tesselslate/waywall) by tesselslate, the
  compositor this is built on.
- [Toolscreen](https://github.com/jojoe77777/Toolscreen) by jojoe77777, the
  Windows tool this is modelled on. No shared code, and toolwall is not
  affiliated with or endorsed by it.
- [Linux MCSR Resources](https://linux-mcsr-resources.github.io/), the
  community index.
- The measuring overlay in `resources/` comes from the waywall generic config
  by Arjun Gore, used under the MIT Licence. The capture geometry, the pie
  chart source rectangles and the god-sens multipliers all come from that
  config.
