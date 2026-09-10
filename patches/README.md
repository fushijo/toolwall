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

## Applying them

    patches/apply.sh ~/waywall

The script skips patches that are already applied, builds, and then asks for
your password once for `ninja install`. If you do not have a checkout:

    git clone https://github.com/tesselslate/waywall ~/waywall
    patches/apply.sh ~/waywall

Written against waywall `150026e`. `git apply` will refuse rather than produce
a mess if the surrounding code has moved.

## Without them

toolwall checks for `waywall.rect` at runtime. If it is missing, the readout
still draws, just with no background and no outline, and the log says so once.
Nothing else in toolwall depends on the patches.

## Licence

waywall is GPL-3.0. These patches are derived from it and are GPL-3.0 as well,
unlike the rest of this repository, which is MIT. Nothing under `patches/` is
compiled into or linked against toolwall itself.
