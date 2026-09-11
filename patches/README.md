# waywall patches

toolwall runs on stock waywall. These patches are optional and only affect the
Ninjabrain Bot readout, which needs two things the scene graph does not have.

## What they add

**`waywall.rect{ dst = {x,y,w,h}, color = "#rrggbbaa", depth = n }`**

A filled rectangle. waywall has no fill primitive at all: `image` draws a PNG
as-is (colour keys are a mirror-only feature upstream, so a white PNG stays
white), and `text` draws glyphs. Without this there is no way to put a panel
behind the readout, which is the whole difference between it looking like a
tool and looking like coloured text lying on the game.

Implemented by reusing the existing texcopy shader with a 1x1 white texture and
a colour key that always matches, so it adds no new shader program.

**`outline` and `outline_color` on `waywall.text`**

Draws each glyph eight times, offset by the outline width, behind the glyph
itself. Text over Minecraft is unreadable in half the biomes without it.

Both are additive: nothing in stock waywall behaves differently, and a config
that does not ask for them produces byte-identical output.

## 0002: hide Ninjabrain Bot's window

**`theme.ninb_hidden = true`**

`show_floating()` is global: there is one visibility flag for every floating
window, so opening any editor also reveals Ninjabrain Bot. That is fine when
ninb's window is what you read. It is not fine when you read its HTTP API and
draw the readout yourself, because then the window is only ever in the way.

This keeps ninb's window hidden regardless of the global flag, matched on its
title. The anchor deliberately is not used for that: it belongs to whichever
floating window opened first, which is ninb only by habit, so close ninb and
the next window to open inherits the slot. Keying the hide off the anchor hides
whatever landed there, which in practice means hiding the editor.

An X11 window's title can arrive after it is mapped, so visibility is applied
again on resize and on config reload rather than only when the window appears.
A window whose title never matches stays visible, which is the safe way round
to be wrong.

It also gives the anchor to ninb and to nothing else. `theme.ninb_anchor` is a
position for ninb, but upstream hands the slot to whichever floating window
opened first, which is ninb only while ninb is that window. Close it and an
editor inherits the slot, and an anchored window cannot be shift-dragged, so it
ends up pinned in a corner with no way to move it.

Until ninb has a title to match, nothing is anchored, which leaves it merely
unpositioned rather than stuck. Anchoring is re-evaluated on every floating
resize for the same reason the hiding is.

## Applying them

    patches/apply.sh ~/waywall

The script skips patches that are already applied, builds, and then asks for
your password once for `ninja install`. If you do not have a checkout:

    git clone https://github.com/tesselslate/waywall ~/waywall
    patches/apply.sh ~/waywall

Written against waywall `150026e`. `git apply` will refuse rather than produce
a mess if the surrounding code has moved.

Building waywall from scratch needs meson 1.4 or newer, since it is C23. If you
already have a configured `build/` directory the script reuses it.

## Without them

toolwall checks for `waywall.rect` at runtime. If it is missing, the readout
still draws, just with no background, no outline and no separators, and the log
says so once. `theme.ninb_hidden` is an unknown config key to an unpatched
waywall, which ignores it. Nothing else in toolwall depends on the patches.

## Licence

waywall is GPL-3.0. These patches are derived from it and are GPL-3.0 as well,
unlike the rest of this repository, which is MIT. Nothing under `patches/` is
compiled into or linked against toolwall itself.
