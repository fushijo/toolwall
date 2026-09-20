# toolwall
![Editing the Ninjabrain readout with the game running](docs/screenshots/showcase.gif)

A GUI and configuration layer for [waywall](https://github.com/tesselslate/waywall),
the Wayland compositor for Minecraft speedrunning on Linux.

> Inspired by [Toolscreen](https://github.com/jojoe77777/Toolscreen), which is
> Windows only. There was no equivalent on Linux, so this is an attempt at one.
> Plenty more is planned over the coming days.
> I used Claude to help debug this and to write the code comments (pls forgiv)

waywall is configured by hand-writing Lua. toolwall turns that configuration
into data, a single JSON file, and gives you a CLI and an in-game editor to
change it, with edits applying live.

## The problem

Hand-writing Lua is fine until you want to move an overlay four pixels left.
Then you are editing lua and trying to get it perfect.

Windows runners have Toolscreen for this. I dont think there is one for Linux

## What you get

- **A config file instead of a program.** One JSON document, so a GUI can edit
  it safely and you can share it without shipping someone executable Lua.
- **An editor you open in game.** `Ctrl+I`, which works rly well just like toolscreen.
- **Your existing config comes with you.** The install reads generic configs and converts it into toolwall friendly.
- **A CLI**, for scripting or when you would rather type (not recommended for general use cases)

## Screenshots

### Overlays in play

![Thin mode with the measuring window, entity counter, pie chart and readout](docs/screenshots/05-overlays-in-play.png)

Measuring window, entity counter, pie chart and the Ninjabrain readout, all
drawn by toolwall.

### Ninjabrain readout

![The Ninjabrain tab, with the readout drawn into the scene](docs/screenshots/06-ninb-readout.png)

The readout is scene text, not a window, this would need the waywall patch to run correctly.

### Keybinds

![The Keybinds tab](docs/screenshots/07-editor-keybinds.png)

Every key, what it does, and whether F3 should suppress it.

### Modes

![The Modes tab with Thin BT expanded](docs/screenshots/02-editor-modes.png)

Resolution, optional sensitivity override, and which overlays come up with it.

### Mirrors

![The Mirrors tab with the entity counter expanded](docs/screenshots/03-editor-mirrors.png)

Rectangles have -/+ steppers, because placing an overlay is nudging rather than
typing.

### Theme

![The Theme tab, with Ninjabrain Bot open beside it](docs/screenshots/04-editor-theme.png)

The editor styles itself from the same config: opacity, dark or light, font.

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

You should just use the tarball in the releases though.

That is it. Launch your instance and hit `Ctrl+I`.

If `toolwall` comes back as "command not found", cargo put it in
`~/.cargo/bin` and your shell is not looking there, run:

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
and on hand-written ones. A waywall config is technically a program, so it
cannot just be parsed: the importer runs it against a fake `waywall` module
that writes down what the config tried to build. Including all the keybinds in the
same sandbox and sorted by what they attempted, so a key reaching for
`set_resolution(340, 1080)` is your thin key whatever it was called.

Anything it cannot map is listed at the end and left out rather than guessed
at. That list also goes in `~/.config/waywall/toolwall-import-report.txt`, so
"what did it drop" is still answerable a week later when you finally reach for
the key that is missing.

Your config has no reason to bind a toolwall command, so the import adds one:
`Ctrl-I` opens the editor, or another key if yours was already using that. The
report says which.

`TOOLWALL_NO_IMPORT=1 ./install.sh` skips it.

### Taking it back out

```sh
./uninstall.sh
```

Puts your old `init.lua` back from the copy the install made and removes the
runtime. Your `toolwall.json` is kept unless you pass `--all`.

[gore]: https://github.com/arjuncgore/waywall_generic_config

## Using it

`Ctrl+I` opens the editor over the game. Edits apply about a third of a second
after you stop making them, so there is no save button, and the mode you are in
survives the reload. `Esc` closes it.

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

## Placing overlays

The Screen tab is a picture of waywall's window with every overlay on it. Drag
to move, corners to resize, and it snaps to edges, centres and the other
overlays with guides while you drag. Hold Alt to ignore the snapping.

Works for mirrors like the pie chart and entity counter, for overlay images
like the eye grid, and for the Ninjabrain readout. None of it needs the waywall
patch: positions and sizes are ordinary config fields. The patch only changes
how the readout's panel is drawn, not where it sits.

Bind a key to `screen.edit` and you get the same thing over the game itself:
a window the size of waywall's with nothing painted behind it, so you drag the
outline of a mirror across the actual mirror. Esc closes it. The tab is still
the better place to set things up before launching, or from a second monitor.

Set the screen size to match waywall's window before you start. Overlay
coordinates are window pixels rather than monitor pixels, and nothing reports
the size, so read it off the `Display:` line in F3.

## Keyboard layouts

The Layout tab is [xkbedit](https://xkbedit.github.io/) built into the editor,
so you can use it in game. Click a key, say what it should type.

Worth knowing: this is not the same as a rebind. A rebind changes *which key*
the game receives, always. A layout changes *which character* a key types, and
only where typing actually happens. So you can put an umlaut on `AltGr+Q` and
`Q` still drops items.

Still working on this so expect bugs.

## Ninjabrain Bot

toolwall can launch it, anchor it, and draw its readout into the scene as text
instead of a floating window. The readout needs ninb's API turned on (Settings
-> Advanced -> "Enable API", port 52533).

Its own hotkeys are global X11 grabs, so toolwall cannot bind them, but the
Ninjabrain tab edits them directly in ninb's preferences file.

## Measuring window

A mirror of the centre of the screen at tall resolution, with a grid overlay on
top, for measuring eye throws.

The capture has to be **60 game pixels wide** (18 grid bands, one band per game
pixel) and the destination width a **multiple of 60**, or the bands land on
fractional screen pixels and drift a couple of pixels end to end. 960, 720 and
600 are exact. 980 is not.

## Patching waywall

Optional, and only the Ninjabrain overlay uses it.

```sh
patches/apply.sh ~/waywall
```

Adds `waywall.rect` (a solid fill), an outline on scene text, and
`theme.ninb_hidden`. None of them can be done from Lua. toolwall checks for
them at runtime and does without if they are missing, so an unpatched waywall
is fine.

## When something is broken

toolwall writes its complaints to `~/.local/state/toolwall.log`. Keybinds it
had to drop, files it could not find, commands that failed. waywall itself
logs to stderr and nowhere else, so if you start your instance from a desktop
entry that log is the only thing that survives.

If you are reporting a bug, that file plus your `toolwall.json` is everything
I need.

```sh
toolwall validate
```

also lists whatever is wrong with the config without applying it.

## Planned

- Per-mode HUD text, and more text placeholders.
- Presets you can drop in, for people who do not want to build a config.
- A wall/projector layout (dont think its possible unless waywall updates)
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

MIT

## Credits

- [waywall](https://github.com/tesselslate/waywall) by tesselslate, the
  compositor this is built on.
- [Toolscreen](https://github.com/jojoe77777/Toolscreen) by jojoe77777, the
  Windows tool this is modelled on.
- [Linux MCSR Resources](https://linux-mcsr-resources.github.io/), the
  community index.
- [waywall_generic_config](https://github.com/arjuncgore/waywall_generic_config)
  by gore. The capture geometry for the pie chart and entity counter
- [Pixel-Perfect Tools](https://priffin.github.io/Pixel-Perfect-Tools/overlayGen.html)
  by priffin, for generating measuring overlays.
