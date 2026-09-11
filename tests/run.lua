--[[
    toolwall runtime tests.

    Run with:  lua tests/run.lua   (from the repository root)
]]

package.path = table.concat({
    "tests/mock/?.lua",
    "runtime/?.lua",
    package.path,
}, ";")

local waywall = require("waywall")

-- Stand in for /proc, so a launched process can be "alive" with a chosen
-- command line. Applied once: launch.lua keeps no state of its own.
local launch = require("toolwall.launch")
local tw_state = require("toolwall.state")
launch.proc_cmdline = function(pid)
    return waywall.processes[pid]
end

-- The readout writes cache and stream scripts into XDG_RUNTIME_DIR. Point them
-- somewhere disposable so running tests cannot disturb a live session.
local ninb_api = require("toolwall.ninb_api")
local TEST_RUNTIME = os.tmpname()
os.remove(TEST_RUNTIME)
os.execute("mkdir -p '" .. TEST_RUNTIME .. "'")
ninb_api.runtime_dir = function()
    return TEST_RUNTIME
end

local passed, failed = 0, 0

local function check(name, fn)
    waywall.reset()

    -- Modules cache runtime state, so reload them for every test.
    for _, mod in ipairs({
        "toolwall", "toolwall.config", "toolwall.scene", "toolwall.modes",
        "toolwall.hud", "toolwall.keybinds", "toolwall.commands",
    }) do
        package.loaded[mod] = nil
    end

    local ok, err = pcall(fn)
    if ok then
        passed = passed + 1
        print("  pass  " .. name)
    else
        failed = failed + 1
        print("  FAIL  " .. name .. "\n        " .. tostring(err))
    end
end

local function assert_eq(actual, expected, label)
    if actual ~= expected then
        error(("%s: expected %s, got %s")
            :format(label or "value", tostring(expected), tostring(actual)), 2)
    end
end

local function write_config(body)
    local path = os.tmpname()
    local fh = assert(io.open(path, "w"))
    fh:write(body)
    fh:close()
    return path
end

local MINIMAL = [[
{
  "version": 1,
  "input": { "sensitivity": 1.0 },
  "mirrors": [
    { "id": "m1", "src": {"x":0,"y":0,"w":10,"h":10}, "dst": {"x":0,"y":0,"w":10,"h":10} }
  ],
  "modes": [
    { "id": "thin", "resolution": {"width":320,"height":1080}, "mirrors": ["m1"] },
    { "id": "wide", "resolution": {"width":1920,"height":300} }
  ],
  "text": [
    { "id": "hud", "template": "{mode} {res}", "x": 8, "y": 8 }
  ],
  "keybinds": [
    { "input": "Shift-T", "command": "mode.set", "args": { "mode": "thin" } },
    { "input": "Shift-R", "command": "mode.reset" }
  ]
}
]]

print("toolwall runtime tests\n")

check("setup returns a waywall config table without touching the scene", function()
    local path = write_config(MINIMAL)
    local toolwall = require("toolwall")

    local cfg = toolwall.setup({ path = path })

    assert_eq(type(cfg), "table", "config type")
    assert_eq(type(cfg.actions), "table", "actions type")
    assert_eq(cfg.input.sensitivity, 1.0, "sensitivity")
    assert_eq(waywall.live_count(), 0, "scene objects created during startup")
    os.remove(path)
end)

check("scene objects appear only after the load event", function()
    local path = write_config(MINIMAL)
    local toolwall = require("toolwall")
    toolwall.setup({ path = path })

    assert_eq(waywall.live_count(), 0, "objects before load")
    waywall.finish_startup()

    -- Base state: no mode mirrors, but the HUD text is drawn.
    assert_eq(waywall.live_count(), 1, "objects after load")
    os.remove(path)
end)

check("mode.set applies resolution and activates its mirrors", function()
    local path = write_config(MINIMAL)
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    cfg.actions["Shift-T"]()

    assert_eq(waywall.resolution.width, 320, "width")
    assert_eq(waywall.resolution.height, 1080, "height")
    assert_eq(toolwall.rt.modes.current, "thin", "current mode")

    local mirrors = 0
    for _, obj in pairs(waywall.live_objects()) do
        if obj.kind == "mirror" then mirrors = mirrors + 1 end
    end
    assert_eq(mirrors, 1, "live mirrors")
    os.remove(path)
end)

check("pressing the same mode again toggles back to base", function()
    local path = write_config(MINIMAL)
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    cfg.actions["Shift-T"]()
    cfg.actions["Shift-T"]()

    assert_eq(waywall.resolution.width, 0, "width after toggle off")
    assert_eq(toolwall.rt.modes.current, nil, "current mode after toggle off")

    local mirrors = 0
    for _, obj in pairs(waywall.live_objects()) do
        if obj.kind == "mirror" then mirrors = mirrors + 1 end
    end
    assert_eq(mirrors, 0, "mirrors closed on toggle off")
    os.remove(path)
end)

check("switching modes closes the previous mode's scene objects", function()
    local path = write_config(MINIMAL)
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    cfg.actions["Shift-T"]()
    toolwall.rt.modes:set("wide")

    local mirrors = 0
    for _, obj in pairs(waywall.live_objects()) do
        if obj.kind == "mirror" then mirrors = mirrors + 1 end
    end
    assert_eq(mirrors, 0, "stale mirrors left live")
    os.remove(path)
end)

check("HUD re-renders on mode change without leaking text objects", function()
    local path = write_config(MINIMAL)
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    cfg.actions["Shift-T"]()
    cfg.actions["Shift-R"]()
    toolwall.rt.modes:set("wide")

    local texts = 0
    for _, obj in pairs(waywall.live_objects()) do
        if obj.kind == "text" then texts = texts + 1 end
    end
    assert_eq(texts, 1, "live text objects")
    os.remove(path)
end)

check("a mode referencing an unknown mirror is rejected at load", function()
    local path = write_config([[
      { "version": 1,
        "modes": [ { "id": "x", "resolution": {"width":0,"height":0}, "mirrors": ["nope"] } ] }
    ]])
    local toolwall = require("toolwall")

    local ok, err = pcall(toolwall.setup, { path = path })
    assert_eq(ok, false, "setup should fail")
    assert_eq(tostring(err):match("unknown mirror") ~= nil, true, "error mentions the cause")
    os.remove(path)
end)

check("a wrong schema version is rejected", function()
    local path = write_config('{ "version": 99 }')
    local toolwall = require("toolwall")

    local ok = pcall(toolwall.setup, { path = path })
    assert_eq(ok, false, "setup should fail")
    os.remove(path)
end)

check("a corrupt config falls back to last known good", function()
    local path = write_config(MINIMAL)

    -- First load succeeds and snapshots the config.
    local toolwall = require("toolwall")
    toolwall.setup({ path = path })

    -- Simulate a bad write from the GUI.
    local fh = assert(io.open(path, "w"))
    fh:write("{ this is not json")
    fh:close()

    package.loaded["toolwall"] = nil
    package.loaded["toolwall.config"] = nil
    waywall.reset()

    local reloaded = require("toolwall")
    local cfg = reloaded.setup({ path = path })

    assert_eq(type(cfg), "table", "fallback config")
    assert_eq(reloaded.rt.degraded ~= nil, true, "degraded flag set")

    os.remove(path)
    os.remove(path .. ".last-good")
end)

check("a missing PNG degrades to a warning, not a crash", function()
    local path = write_config([[
      { "version": 1,
        "images": [ { "id": "gone", "path": "/nonexistent/x.png",
                      "dst": {"x":0,"y":0,"w":1,"h":1} } ],
        "modes": [ { "id": "m", "resolution": {"width":0,"height":0}, "images": ["gone"] } ] }
    ]])
    local toolwall = require("toolwall")
    toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    -- Must not throw.
    toolwall.rt.modes:set("m")
    assert_eq(toolwall.rt.modes.current, "m", "mode still applied")
    os.remove(path)
end)

check("applying the default mode during load, before the window exists, does not crash", function()
    -- Real waywall fires "load" as soon as its own config parses, which is
    -- several seconds before Minecraft's window connects and maps a view.
    -- toolwall.setup() applies default_mode/reset() synchronously inside the
    -- "load" listener, so this window must not be able to crash it.
    local path = write_config([[
      { "version": 1,
        "default_mode": "m",
        "modes": [ { "id": "m", "resolution": {"width":320,"height":1080} } ] }
    ]])
    local toolwall = require("toolwall")
    toolwall.setup({ path = path })

    -- Must not throw, even though the view is not ready yet.
    waywall.finish_startup()

    assert_eq(toolwall.rt.modes.current, nil, "mode not applied before the view exists")
    assert_eq(waywall.resolution.width, 0, "resolution untouched before the view exists")

    -- Once the window connects, a real keypress applies the mode normally.
    waywall.mount_view()
    toolwall.rt.modes:set("m")

    assert_eq(toolwall.rt.modes.current, "m", "mode applied once the view exists")
    assert_eq(waywall.resolution.width, 320, "resolution applied once the view exists")
    os.remove(path)
end)

check("theme.ninb_anchor of \"\" or unset is omitted, not sent to waywall as an empty string", function()
    -- waywall's real config parser treats "no anchor" as the Lua field being
    -- entirely absent; it rejects an empty string with
    -- "invalid value '' for 'theme.ninb_anchor'" and refuses to load the
    -- whole config. Our schema allows "" as the GUI's "none" dropdown
    -- choice, so the runtime must translate that (and an unset field) into
    -- an omitted key, not a literal "".
    local unset_path = write_config(MINIMAL)
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = unset_path })
    assert_eq(cfg.theme.ninb_anchor, nil, "unset ninb_anchor should be omitted")
    os.remove(unset_path)

    package.loaded["toolwall"] = nil
    package.loaded["toolwall.config"] = nil
    waywall.reset()

    local empty_path = write_config([[
      { "version": 1, "theme": { "ninb_anchor": "" } }
    ]])
    toolwall = require("toolwall")
    cfg = toolwall.setup({ path = empty_path })
    assert_eq(cfg.theme.ninb_anchor, nil, "empty-string ninb_anchor should be omitted")
    os.remove(empty_path)
end)

check("gui.toggle expands ~ in gui.command before exec", function()
    -- waywall.exec() is a bare execvp() using the compositor's own PATH,
    -- which commonly lacks ~/.cargo/bin unless waywall was launched from an
    -- interactive shell. gui.command must be expanded the same way image
    -- and shader paths are.
    local path = write_config([[
      { "version": 1,
        "modes": [ { "id": "m", "resolution": {"width":0,"height":0} } ],
        "keybinds": [ { "input": "Ctrl-I", "command": "gui.toggle" } ],
        "gui": { "command": "~/bin/toolwall-gui" } }
    ]])
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    cfg.actions["Ctrl-I"]()

    assert_eq(waywall.launched[1], (os.getenv("HOME") or "") .. "/bin/toolwall-gui",
        "expanded command inside the launcher")
    os.remove(path)
end)

check("gui.toggle forces a visibility transition so waywall commits the window", function()
    -- waywall's floating_set_visible() early-returns when the flag already
    -- matches, and that function is the only one that commits the views. A
    -- window mapped while the flag was already true is therefore "visible"
    -- and absent from the screen until something drives a real transition.
    local path = write_config([[
      { "version": 1,
        "modes": [ { "id": "m", "resolution": {"width":0,"height":0} } ],
        "keybinds": [ { "input": "Ctrl-I", "command": "gui.toggle" } ],
        "gui": { "command": "toolwall-gui" } }
    ]])
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    -- Something already revealed the floating layer, so a bare
    -- show_floating(true) would be a no-op inside waywall.
    waywall.show_floating(true)

    cfg.actions["Ctrl-I"]()

    local transitions = {}
    for _, entry in ipairs(waywall.log) do
        if entry.name == "show_floating" then
            table.insert(transitions, entry.args[1])
        end
    end

    assert_eq(transitions[#transitions - 1], false, "forced hide before show")
    assert_eq(transitions[#transitions], true, "shown after the forced transition")
    assert_eq(waywall.floating, true, "floating layer visible")
    os.remove(path)
end)

check("ninb.toggle launches Ninjabrain Bot and reveals it", function()
    local path = write_config([[
      { "version": 1,
        "modes": [ { "id": "m", "resolution": {"width":0,"height":0} } ],
        "ninb": { "jar": "~/ninb.jar", "command": "java -jar {jar}" },
        "keybinds": [ { "input": "grave", "command": "ninb.toggle" } ] }
    ]])
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    cfg.actions["grave"]()

    assert_eq(waywall.launched[1],
        "java -jar " .. (os.getenv("HOME") or "") .. "/ninb.jar",
        "{jar} substituted and ~ expanded")
    assert_eq(waywall.floating, true, "revealed after launch")

    cfg.actions["grave"]()
    assert_eq(waywall.floating, false, "second press hides")
    assert_eq(#waywall.launched, 1, "launched only once")
    os.remove(path)
end)

check("ninb.toggle without a jar does not consume the keypress", function()
    local path = write_config([[
      { "version": 1,
        "modes": [ { "id": "m", "resolution": {"width":0,"height":0} } ],
        "keybinds": [ { "input": "grave", "command": "ninb.toggle" } ] }
    ]])
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    assert_eq(cfg.actions["grave"](), false, "passes the key through")
    os.remove(path)
end)

check("a second press while starting does not launch a rival copy", function()
    -- The pid is recorded by the launcher script, which cannot have run yet
    -- when exec returns. A press in that window used to see "not running" and
    -- start another Ninjabrain Bot.
    local path = write_config([[
      { "version": 1,
        "modes": [ { "id": "m", "resolution": {"width":0,"height":0} } ],
        "ninb": { "jar": "~/ninb.jar", "command": "java -jar {jar}" },
        "keybinds": [ { "input": "grave", "command": "ninb.toggle" } ] }
    ]])
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    cfg.actions["grave"]()

    -- Drop the pid, so only the claim can hold the second press off.
    os.remove(launch.pid_path("ninb"))

    cfg.actions["grave"]()
    assert_eq(#waywall.launched, 1, "still exactly one Ninjabrain Bot")
    os.remove(path)
end)

check("the active mode survives a config reload", function()
    -- Saving reloads the config, which rebuilds the VM. Live editing is only
    -- usable if that does not throw you back to the base resolution.
    local body = [[
      { "version": 1,
        "modes": [ { "id": "thin", "resolution": {"width":320,"height":1080} } ] }
    ]]
    local path = write_config(body)

    local toolwall = require("toolwall")
    toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    toolwall.rt.modes:set("thin")
    assert_eq(waywall.resolution.width, 320, "mode applied")

    for _, mod in ipairs({ "toolwall", "toolwall.config", "toolwall.scene",
                           "toolwall.modes", "toolwall.hud", "toolwall.keybinds",
                           "toolwall.commands" }) do
        package.loaded[mod] = nil
    end
    waywall.resolution = { width = 0, height = 0 }

    local reloaded = require("toolwall")
    reloaded.setup({ path = path })
    waywall.fire("load")

    assert_eq(reloaded.rt.modes.current, "thin", "still in thin after a reload")
    assert_eq(waywall.resolution.width, 320, "resolution reapplied")
    os.remove(path)
end)

check("a keybind does not fire while F3 is held", function()
    -- waywall matches modifiers exactly, so Shift/Ctrl/Alt are already safe.
    -- F3 is an ordinary key, so without a guard F3+B would flip you into thin
    -- while you were only reaching for hitboxes.
    local path = write_config([[
      { "version": 1,
        "modes": [ { "id": "thin", "resolution": {"width":320,"height":1080} } ],
        "keybinds": [ { "input": "B", "command": "mode.set",
                        "args": { "mode": "thin" } },
                      { "input": "N", "command": "mode.set",
                        "args": { "mode": "thin" }, "f3_safe": false } ] }
    ]])
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = path })
    waywall.finish_startup()
    waywall.mount_view()

    waywall.held["F3"] = true

    assert_eq(cfg.actions["B"](), false, "passes F3+B through to Minecraft")
    assert_eq(toolwall.rt.modes.current, nil, "mode not switched")

    -- Opting out still fires, for a bind that wants to work during F3.
    cfg.actions["N"]()
    assert_eq(toolwall.rt.modes.current, "thin", "f3_safe = false still fires")

    -- And without F3 held the guarded bind works normally.
    toolwall.rt.modes:reset()
    waywall.held["F3"] = false
    cfg.actions["B"]()
    assert_eq(toolwall.rt.modes.current, "thin", "fires normally without F3")
    os.remove(path)
end)

check("unknown commands do not consume the keypress", function()
    local path = write_config([[
      { "version": 1,
        "modes": [ { "id": "m", "resolution": {"width":0,"height":0} } ],
        "keybinds": [ { "input": "Shift-Q", "command": "does.not.exist" } ] }
    ]])
    local toolwall = require("toolwall")
    local cfg = toolwall.setup({ path = path })
    waywall.finish_startup()

    assert_eq(cfg.actions["Shift-Q"](), false, "should return false")
    os.remove(path)
end)

--[[
    The ninb readout. These run against a real API response, so the field names
    are the ones ninb 1.5.2 actually sends.
]]

local NINB_RESPONSE = [==[
  { "resultType": "TRIANGULATION",
    "playerPosition": { "xInOverworld": -214.71, "zInOverworld": 196.78,
                        "horizontalAngle": -16.41, "isInOverworld": true },
    "predictions": [
      { "chunkX": 14, "chunkZ": 106, "certainty": 0.849, "overworldDistance": 1567.0 },
      { "chunkX": 22, "chunkZ": 133, "certainty": 0.149, "overworldDistance": 2132.0 } ],
    "eyeThrows": [
      { "xInOverworld": -214.71, "zInOverworld": 196.78,
        "angle": -16.49, "angleWithoutCorrection": -16.4944,
        "correctionIncrements": 2, "error": 0.0002 } ] }
]==]

local function ninb_data()
    return require("toolwall.json").decode(NINB_RESPONSE)
end

local function drawn(cfg, data, messages)
    waywall.reset()
    waywall.finish_startup()

    package.loaded["toolwall.ninb_overlay"] = nil
    local overlay = require("toolwall.ninb_overlay")

    overlay.new(cfg):draw(data or ninb_data(), 1000, messages)

    local text, rects = {}, {}
    for _, entry in ipairs(waywall.log) do
        if entry.name == "text" then
            table.insert(text, { body = entry.args[1], options = entry.args[2] })
        elseif entry.name == "rect" then
            table.insert(rects, entry.args[1])
        end
    end
    return text, rects
end

local function joined(text)
    local out = {}
    for _, item in ipairs(text) do table.insert(out, item.body) end
    return table.concat(out, "|")
end

check("the readout converts a stronghold chunk to block and nether coordinates", function()
    local api = require("toolwall.ninb_api")
    local data = ninb_data()

    local fields = api.prediction_fields(data.predictions[1], data, "block")

    -- chunk 14 is blocks 228..243, and ninb points at the centre
    assert_eq(fields.blockX, "228", "block x")
    assert_eq(fields.blockZ, "1700", "block z")

    -- the nether is an eighth of the scale, which is the coordinate a run
    -- actually travels to
    assert_eq(fields.netherX, "28", "nether x")
    assert_eq(fields.netherZ, "212", "nether z")
    assert_eq(fields.netherDistance, "195", "nether distance")
    assert_eq(fields.certainty, "84.9%", "certainty")
end)

check("the readout works out which way to turn", function()
    local api = require("toolwall.ninb_api")
    local data = ninb_data()

    local fields = api.prediction_fields(data.predictions[1], data, "block")

    -- minecraft yaw: 0 faces +z and decreases anticlockwise
    assert_eq(fields.angle, "-16.41", "heading to the stronghold")
    assert_eq(fields.angleDelta, "0.0", "already pointing there, and not '-0.0'")
end)

check("the ninbot layout lays labels and values out in columns", function()
    local text = drawn({
        layout = "ninbot", x = 8, y = 40, size = 2, line_gap = 2, coords = "block",
        show_location = true, show_certainty = true, show_nether = true,
        show_distance = true, background = false,
    })

    assert_eq(joined(text),
        "Location:|(228, 1700), 1567 blocks away|" ..
        "Certainty:|84.9%|" ..
        "Nether coords:|(28, 212), 195 blocks away",
        "three labelled rows")

    -- every value starts in the same column, one past the widest label
    assert_eq(text[2].options.x, text[4].options.x, "values share a column")
    assert_eq(text[2].options.x, text[6].options.x, "values share a column")

    -- rows cannot overlap: the pitch clears the 16px glyph at size 2
    assert_eq(text[3].options.y - text[1].options.y, 34, "row pitch")
end)

check("certainty is coloured by how good it is", function()
    local cfg = {
        layout = "compact", shown_predictions = 2, coords = "block",
        template = "{x}, {z} {certainty}", background = false,
        certainty_high_above = 80, certainty_mid_above = 50,
        certainty_high_color = "#55ff55ff", certainty_low_color = "#ff5555ff",
    }
    local text = drawn(cfg)

    assert_eq(text[1].options.color, "#55ff55ff", "84.9% is high")
    assert_eq(text[2].options.color, "#ff5555ff", "14.9% is low")
end)

check("the panel is measured from the text, not guessed", function()
    local _, rects = drawn({
        layout = "compact", shown_predictions = 1, coords = "block",
        template = "{x}, {z}", background = true, padding = 8, size = 2,
        line_gap = 2, x = 100, y = 100,
    })

    assert_eq(#rects, 1, "one panel, no border")

    -- "228, 1700" is 9 characters of an 8px face at size 2
    assert_eq(rects[1].dst.w, 9 * 8 * 2 + 8 * 2, "panel width")
    assert_eq(rects[1].dst.h, 16 * 2 + 8 * 2, "panel height")
    assert_eq(rects[1].dst.x, 100 - 8, "panel starts a padding left of the text")
end)

check("the readout says something before the first throw", function()
    local text = drawn({ layout = "ninbot", idle_text = "no eye throws yet" },
        { predictions = {}, eyeThrows = {} })

    assert_eq(joined(text), "no eye throws yet", "idle line")
end)

check("eye throws keep the precision ninb reports them with", function()
    local text = drawn({
        layout = "ninbot", show_location = false, show_certainty = false,
        show_nether = false, throw_rows = 3, show_throw_header = false,
        background = false,
    })

    assert_eq(joined(text), "-214.71|196.78|-16.49+2|0.0002", "one throw, four columns")
end)

check("a nudged throw shows what was measured and how far it moved", function()
    -- ninb does not rewrite the angle when you nudge it, it counts the
    -- increments, and that count is what you need to decide whether to nudge
    -- again. "-16.49+2" is the measurement plus two nudges up.
    local cfg = {
        layout = "ninbot", show_location = false, show_certainty = false,
        show_nether = false, throw_rows = 1, show_throw_header = false,
        background = false,
    }
    local text = drawn(cfg)
    assert_eq(text[3].body, "-16.49+2", "measured angle and increments")

    cfg.show_correction = false
    text = drawn(cfg)
    assert_eq(text[3].body, "-16.49", "corrected angle when nudges are off")
end)

check("separators take up room instead of overlapping the rows", function()
    local cfg = {
        layout = "ninbot", show_location = true, show_certainty = false,
        show_nether = false, show_distance = false, throw_rows = 1,
        show_throw_header = false, background = true, padding = 0,
        size = 2, line_gap = 2, separator_width = 3, x = 0, y = 0,
    }

    local plain, plain_rects = drawn(cfg)
    cfg.separators = true
    local ruled, ruled_rects = drawn(cfg)

    assert_eq(#ruled_rects, #plain_rects + 1, "one rule between the two sections")

    -- the rule is 3px tall with a 2px gap under it, so everything below it
    -- moves down by 5 and the panel grows by the same
    local function last_y(list)
        return list[#list].options.y
    end
    assert_eq(last_y(ruled) - last_y(plain), 5, "rows below the rule move down")

    local function panel(list)
        for _, r in ipairs(list) do
            if r.dst.h > 6 then return r end
        end
    end
    assert_eq(panel(ruled_rects).dst.h - panel(plain_rects).dst.h, 5, "the panel grows to fit it")
end)

check("an unchanged readout is not redrawn", function()
    -- text objects cannot be mutated, so a redraw closes and recreates every
    -- object. At a 50ms tick that has to not happen unless something moved.
    waywall.reset()
    waywall.finish_startup()

    package.loaded["toolwall.ninb_overlay"] = nil
    local overlay = require("toolwall.ninb_overlay")

    local panel = overlay.new({ layout = "ninbot", background = false })
    local data = ninb_data()

    panel:draw(data, 1000)
    local first = waywall.live_count()
    assert_eq(first > 0, true, "something was drawn")

    panel:draw(data, 1050)
    assert_eq(waywall.live_count(), first, "same readout, same objects")

    data.predictions[1].chunkX = 15
    panel:draw(data, 1100)
    assert_eq(waywall.live_count(), first, "a changed readout is rebuilt, not stacked")
end)

check("the readout starts on its own when it is enabled", function()
    -- with ninb's own window hidden the readout is the only thing showing the
    -- stronghold, so it cannot wait for a keypress
    local path = write_config([[
      { "version": 1,
        "modes": [ { "id": "m", "resolution": {"width":0,"height":0} } ],
        "ninb": { "jar": "~/ninb.jar", "overlay": { "enabled": true } } }
    ]])
    local toolwall = require("toolwall")
    toolwall.setup({ path = path })

    assert_eq(toolwall.rt.ninb_overlay, nil, "not running before load")

    -- finish_startup fires the load event, which is what starts it. the loop
    -- only ends when a sleep fails, which is what teardown looks like.
    waywall.sleep_budget = 2
    waywall.finish_startup()

    local sleeps = 0
    for _, entry in ipairs(waywall.log) do
        if entry.name == "sleep" then sleeps = sleeps + 1 end
    end
    assert_eq(sleeps, 3, "the loop ran until its sleeps ran out")

    os.remove(path)
end)

check("the readout stays off when it is not enabled", function()
    local path = write_config([[
      { "version": 1,
        "modes": [ { "id": "m", "resolution": {"width":0,"height":0} } ] }
    ]])
    local toolwall = require("toolwall")
    toolwall.setup({ path = path })
    waywall.sleep_budget = 2
    waywall.finish_startup()

    assert_eq(toolwall.rt.ninb_overlay, nil, "nothing started")
    os.remove(path)
end)

check("the readout keeps trying to stream instead of giving up on it", function()
    -- ninb takes seconds to boot, so the first attempt after a reload nearly
    -- always fails. giving up on that would mean never streaming at all.
    local path = write_config([[
      { "version": 1,
        "modes": [ { "id": "m", "resolution": {"width":0,"height":0} } ],
        "ninb": { "jar": "~/ninb.jar",
                  "overlay": { "enabled": true, "live": true, "poll_ms": 16 } } }
    ]])
    local toolwall = require("toolwall")
    toolwall.setup({ path = path })

    -- long enough to pass the old three second give-up
    waywall.sleep_budget = 4000
    waywall.finish_startup()

    local streams, fetches = 0, 0
    for _, entry in ipairs(waywall.log) do
        if entry.name == "exec" then
            local command = tostring(entry.args[1])
            if command:match("%-ninb%-stronghold%.sh$") then
                streams = streams + 1
            elseif command:match("^curl") then
                fetches = fetches + 1
            end
        end
    end

    assert_eq(streams > 1, true, "the stream is started again, not abandoned")
    assert_eq(fetches > 0, true, "and fetches cover the gap meanwhile")

    os.remove(path)
end)

print(("\n%d passed, %d failed"):format(passed, failed))

os.exit(failed == 0 and 0 or 1)
