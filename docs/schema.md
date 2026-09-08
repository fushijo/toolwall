# Config reference

`schema/toolwall.schema.json` is the machine-readable contract. This page
covers the parts that need prose.

## Commands and their arguments

| Command | Arguments | Notes |
| --- | --- | --- |
| `mode.set` | `{ "mode": "<id>" }` | Repressing toggles back to base if the mode has `toggle: true`. |
| `mode.reset` | — | Stretch to window, default sensitivity, base overlays only. |
| `mode.cycle` | `{ "modes": ["a","b"] }` | Omit `modes` to cycle all, in document order. Always advances; never toggles off. |
| `sens.set` | `{ "sensitivity": 0.6 }` | `0` restores `input.sensitivity`. |
| `keymap.set` | `{ "layout": "de", ... }` | For search crafting in another language. |
| `remaps.set` | `{ "remaps": { "X": "F3" } }` | Replaces the active remap set wholesale. |
| `key.press` | `{ "key": "F3" }` | Synthetic keypress into Minecraft. |
| `fullscreen.toggle` | — | For compositors without undecorated fullscreen. |
| `floating.toggle` / `.show` / `.hide` | — | Global across all floating windows. |
| `gui.toggle` | — | Launches `gui.command` on first use, then toggles visibility. |
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

## Text templates

| Placeholder | Value |
| --- | --- |
| `{mode}` | Current mode's label, or `hud.idle_label` when in base state |
| `{res}` | `WxH`, or `auto` when unset |
| `{width}` / `{height}` | Individually |
| `{sens}` | Effective sensitivity |
| `{state}` | Instance state, e.g. `inworld/paused` — requires State Output |

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
