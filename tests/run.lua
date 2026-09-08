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

print(("\n%d passed, %d failed"):format(passed, failed))
os.exit(failed == 0 and 0 or 1)
