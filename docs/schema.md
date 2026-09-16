# Config reference

`schema/toolwall.schema.json` is the machine-readable contract. This page
covers the parts that need prose.

## Commands and their arguments

| Command | Arguments | Notes |
| --- | --- | --- |
| `mode.set` | `{ "mode": "<id>" }` | Repressing toggles back to base if the mode has `toggle: true`. |
| `mode.reset` | none | Stretch to window, default sensitivity, base overlays only. |
| `mode.cycle` | `{ "modes": ["a","b"] }` | Omit `modes` to cycle all, in document order. Always advances; never toggles off. |
| `sens.set` | `{ "sensitivity": 0.6 }` | `0` restores `input.sensitivity`. |
| `keymap.set` | `{ "layout": "de", ... }` | For search crafting in another language. |
| `remaps.set` | `{ "remaps": { "X": "F3" } }` | Replaces the active remap set wholesale. |
| `key.press` | `{ "key": "F3" }` | Synthetic keypress into Minecraft. |
| `fullscreen.toggle` | none | For compositors without undecorated fullscreen. |
| `floating.toggle` / `.show` / `.hide` | none | Global across all floating windows. |
| `gui.toggle` | none | Launches `gui.command` on first use, then toggles visibility. |
| `ninb.toggle` | none | Launches Ninjabrain Bot from `ninb.jar` on first use, then toggles visibility. |
| `overlay.toggle` | `{ "overlay": "<id>" }` | Pins one mirror or image on top of the current mode until toggled off. |
| `exec` | `{ "command": "..." }` | Requires `gui.allow_exec: true`. |

An unknown command logs a warning and returns `false`, which passes the keypress
through to Minecraft rather than swallowing it.

## Input strings

Straight from waywall. Modifiers are `shift`, `ctrl`, `alt`/`mod1`,
`super`/`mod4`, `caps`, `num`, `mod3`, `mod5`. Buttons are `lmb`, `rmb`, `mmb`,
`mb4`, `mb5`.

An action fires only when the pressed modifiers match **exactly**. `"T"` will
not fire while Shift is held. Use `*` as a wildcard: `"*-T"` fires regardless of
modifiers, `"*-Shift-T"` requires Shift and ignores the rest.

Caps Lock and Num Lock are ignored unless named explicitly.

## Rebind names are not keybind names

`input.remaps`, `input.remaps_menu` and the `remaps.set` arguments use a
different vocabulary from every other input string on this page, and mixing
them up is the single most expensive mistake in a toolwall config.

| | vocabulary | examples |
| --- | --- | --- |
| Keybinds (`keybinds[].input`) | X11 keysyms, modifiers split on `-` | `Escape`, `bracketleft`, `Ctrl-Shift-N`, `*-F3` |
| Rebinds (`input.remaps`) | Linux input-event-code names, matched whole | `ESC`, `LEFTBRACE`, `N`, `F3` |

waywall matches a rebind half against `util_keycodes` and then
`button_mappings`, case-insensitively, with no splitting. So a `-` is never a
separator there and `Ctrl-N` is not a name at all. The two vocabularies agree
on letters, digits and function keys and disagree on everything else, which is
what makes this so easy to get wrong: the first rebinds anyone tries work, and
the first punctuation key does not.

**It does not fail on its own.** `config_parse_remap` returning non-zero aborts
the entire config load, so one bad rebind name costs every mode, mirror and
keybind in the document. The symptom is "toolwall stopped working".

Three things now stand between you and that:

- The editor writes these names for you. Press **Set**, press the key.
- `toolwall validate` rejects a name waywall would not take, and suggests the
  right one where there is an obvious match.
- The runtime drops an unparseable rebind with a warning rather than handing it
  to waywall, so a hand-edited or shared config degrades to "that one rebind is
  missing" instead of "nothing loads".

The full list of accepted names is `schema/keycodes.txt`, extracted from
waywall's own source by `tools/keycodes.sh`.

### Rebinding a modifier on its own

`LEFTALT`, `RIGHTSHIFT`, `LEFTCTRL`, `RIGHTMETA`, `CAPSLOCK` and the rest are
ordinary remap sources and targets. waywall matches a remap against the raw
keycode in `try_remap_key`, before any modifier handling, so a modifier is no
different from `A` as far as rebinding goes:

```json
"remaps": { "LEFTALT": "F3", "RIGHTSHIFT": "mb4" }
```

**Pressing one cannot fill the field, though.** winit merges left and right
into a single flag before egui sees it, and egui has no key value for a
modifier in the first place, so a bare Left Alt press produces no event for the
editor to read. That is a limit of the editor's toolkit, not of waywall.

So the editor offers them by name instead: the **▾** button beside each field
opens waywall's full list, searchable and grouped, with the modifiers first.
Typing the name into the field directly works too.

One thing to know: a keybind on the same key wins. `on_keyboard_key` runs
actions before `try_remap_key` and returns early when one consumes the press,
so binding `*-Alt_L` as a keybind and rebinding `LEFTALT` at the same time
means the keybind is what happens.

## Text templates

| Placeholder | Value |
| --- | --- |
| `{mode}` | Current mode's label, or `hud.idle_label` when in base state |
| `{res}` | `WxH`, or `auto` when unset |
| `{width}` / `{height}` | Individually |
| `{sens}` | Effective sensitivity |
| `{state}` | Instance state, e.g. `inworld/paused`, requires State Output |

A template rendering to an empty string is skipped, so an element can disappear
conditionally.

`{state}` needs `hud.follow_state: true` to refresh on state changes, and that
requires the State Output mod. Without the mod, `waywall.state()` throws; the
HUD catches it and renders an empty string.

## Depth ordering

waywall's ordering, front to back:

1. Positive depth
2. Images with unspecified depth
3. Mirrors with unspecified depth
4. Text with unspecified depth
5. The Minecraft instance
6. Negative depth

HUD text needs an explicit positive depth to sit above mirrors and images.

## Resolution limits

Leaderboard rules permit only **one** resolution extending past your monitor
bounds, and cap any dimension at **16384px**. The schema and
`store::validate` enforce the cap; the single-oversized-resolution rule is on
you.

## Paths

`~` and `$VARS` are expanded by the runtime (`util.expand`). Neither waywall nor
Prism Launcher expand `~`, so an unexpanded path fails silently as "file not
found".

Shader filenames resolve relative to the waywall config directory if not
absolute.
