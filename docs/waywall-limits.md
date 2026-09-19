# waywall's limits

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
- **Scene text has one font and one colour.** The face is the bundled Terminus
  at 8x16, scaled by a whole number, with no way to pick another. toolwall
  works with the grid rather than against it: a column is a character count,
  which is what lets the Ninjabrain readout line up its labels and values.
  Multiple colours on one line means multiple text objects.
- **No fill primitive and no text outline** without the patches in `patches/`.
- **`show_floating` has no per-window form.** `theme.ninb_hidden` in `patches/`
  is the narrow fix: it hides Ninjabrain Bot's window and nothing else.
- **The anchor slot goes to whichever floating window opens first.** That is
  Ninjabrain Bot only by habit. Close it and the editor inherits the slot, and
  an anchored window cannot be shift-dragged, so the editor stops moving until
  ninb takes the slot back. The patch in `patches/` reserves the slot for ninb,
  which is what the option is named for.
- **A child process's stderr goes to waywall's console.** Only stdout is sent
  to `/dev/null` (`subproc_exec`), so anything you `exec()` that complains,
  `curl` being the obvious one, ends up in the log. Silence it in the command.
- **Ninjabrain Bot calibration does not work correctly inside waywall.** Use
  boat eye, or calibrate outside.
