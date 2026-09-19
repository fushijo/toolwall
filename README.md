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

## The problem

Hand-writing Lua is fine until you want to move an overlay four pixels left.
Then you are editing code, restarting the game, squinting at the result, and
doing it again. Change something small, alt-tab, relaunch, look, repeat.

Windows runners have Toolscreen for this. Linux did not have anything.

## What you get

- **A config file instead of a program.** One JSON document, so a GUI can edit
  it safely and you can share it without shipping someone executable Lua.
- **An editor you open in game.** `Ctrl+I`, nudge the thing, watch it move. No
  save button, no restart.
- **It keeps working after a waywall update.** toolwall sits on top rather than
  forking, so a new waywall release does not break it.
- **Your existing config comes with you.** The install reads it and converts it.
- **A CLI**, for scripting or when you would rather type than click.

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

You need waywall working already, plus a Rust toolchain. `luajit` too if you
want to run the tests.

```sh
git clone https://github.com/fushijo/toolwall
cd toolwall
./install.sh
cargo install --path crates/toolwall-cli
cargo install --path crates/toolwall-gui
toolwall validate
```

That is it. Launch your instance and hit `Ctrl+I`.

If `toolwall` comes back as "command not found", cargo put it in
`~/.cargo/bin` and your shell is not looking there:

```sh
echo 'PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
```

### It reads the config you already have

If you already run waywall, the install converts your config instead of
replacing it. Resolutions, mirrors, overlays, rebinds, and the keys you bound
them to all come across.

```
imported 3 mode(s), 5 mirror(s), 4 overlay image(s), 5 keybind(s)
  Thin       340x1080  (2 mirror(s), 1 image(s))
  Tall       384x16384  (3 mirror(s), 2 image(s))
  Wide       1920x300  (0 mirror(s), 1 image(s))
```

It works on [gore's generic config][gore], which is where most people start,
and on hand-written ones. A waywall config is a program, not a document, so it
cannot just be parsed: the importer runs it against a fake `waywall` module
that writes down what the config tried to build. Keybinds get called in that
same sandbox and sorted by what they attempted, so a key reaching for
`set_resolution(340, 1080)` is your thin key whatever it was called.

Anything it cannot map is listed at the end and left out rather than guessed
at. Your old files stay put; `init.lua` becomes a two-line shim and the backup
next to it puts everything back.

`TOOLWALL_NO_IMPORT=1 ./install.sh` skips it.

[gore]: https://github.com/arjuncgore/waywall_generic_config

## Using it

`Ctrl+I` opens the editor over the game. Edits apply about a third of a second
after you stop making them, so there is no save button, and the mode you are in
survives the reload. `Esc` closes it.

Minecraft has to let go of the cursor before you can click a floating window,
so opening the editor presses Escape into the game for you.

From a terminal:

```sh
toolwall validate                              # check without applying
toolwall modes                                 # list modes
toolwall get modes.thin.resolution.width
toolwall set modes.thin.resolution.width 340   # applies live
toolwall reload                                # re-trigger without editing
```

Dotted paths index arrays by `id`, so `modes.thin` keeps working when you
reorder things.

## Keyboard layouts

The Layout tab is [xkbedit](https://xkbedit.github.io/) built into the editor,
so you can use it in game. Click a key, say what it should type.

Worth knowing: this is not the same as a rebind. A rebind changes *which key*
the game receives, always. A layout changes *which character* a key types, and
only where typing actually happens. So you can put an umlaut on `AltGr+Q` and
`Q` still drops items.

Four levels per key (base, Shift, AltGr, Shift+AltGr), and only the keys you
change get written out. AltGr is the roomy one, since nothing lives there on a
US layout. `toolwall layout` writes the file from the command line if you are
setting up a second machine.

## Ninjabrain Bot

toolwall can launch it, anchor it, and draw its readout into the scene as text
instead of a floating window. The readout needs ninb's API turned on (Settings
-> Advanced -> "Enable API", port 52533).

Its own hotkeys are global X11 grabs, so toolwall cannot bind them, but the
Ninjabrain tab edits them directly in ninb's preferences file. Restart ninb
after changing them, since it rewrites that file on exit.

## Measuring window

A mirror of the centre of the screen at tall resolution, with a grid overlay on
top, for measuring eye throws.

The capture has to be **60 game pixels wide** (18 grid bands, one band per game
pixel) and the destination width a **multiple of 60**, or the bands land on
fractional screen pixels and drift a couple of pixels end to end. 960, 720 and
600 are exact. 980 is not.

## Patching waywall

Optional, and only the Ninjabrain readout uses it.

```sh
patches/apply.sh ~/waywall
```

Adds `waywall.rect` (a solid fill), an outline on scene text, and
`theme.ninb_hidden`. None of them can be done from Lua. toolwall checks for
them at runtime and does without if they are missing, so an unpatched waywall
is fine.

## Planned

- Dragging overlay rectangles around instead of typing coordinates.
- Per-mode HUD text, and more text placeholders.
- Presets you can drop in, for people who do not want to build a config.
- A wall/projector layout, if waywall grows the pieces for it.
- Per-window floating control, upstream, so opening the editor stops also
  revealing ninb.

## Developing

```sh
luajit tests/run.lua      # the Lua runtime
cargo test --workspace    # everything else
```

[`docs/schema.md`](docs/schema.md) is the config reference,
[`docs/architecture.md`](docs/architecture.md) covers how the pieces fit, and
[`docs/waywall-limits.md`](docs/waywall-limits.md) lists the waywall
behaviours that shaped the design.

## Licence

MIT.

toolwall is a separate program that talks to waywall through configuration
files. It does not link against or derive from waywall's source, so waywall's
GPL-3.0-only licence does not extend to it.

`patches/` is the exception. Those are diffs against waywall's C source, so
they are derived from it and are GPL-3.0-only. They are not compiled into or
linked against anything here: they are applied to your own waywall checkout,
which you then build yourself. Nothing under `patches/` ships in the toolwall
binaries.

## Credits

- [waywall](https://github.com/tesselslate/waywall) by tesselslate, the
  compositor this is built on.
- [Toolscreen](https://github.com/jojoe77777/Toolscreen) by jojoe77777, the
  Windows tool this is modelled on. No shared code, and toolwall is not
  affiliated with or endorsed by it.
- [Linux MCSR Resources](https://linux-mcsr-resources.github.io/), the
  community index.
- [waywall_generic_config](https://github.com/arjuncgore/waywall_generic_config)
  by gore. The capture geometry for the pie chart and entity counter, and the
  god-sens multipliers, are taken from that config.
- [Pixel-Perfect Tools](https://priffin.github.io/Pixel-Perfect-Tools/overlayGen.html)
  by priffin, for generating measuring overlays.
