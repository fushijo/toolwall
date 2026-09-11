--[[
    toolwall, a data-driven configuration layer for waywall.

    Your ~/.config/waywall/init.lua becomes:

        local toolwall = require("toolwall")
        return toolwall.setup()

    Everything else lives in toolwall.json, which the GUI and CLI edit.

    IMPORTANT LIFECYCLE NOTE
    ------------------------
    waywall forbids calling most of its API "during startup" (set_resolution,
    mirror, image, text, exec, ...). Only the returned config table may be
    built synchronously. Everything that touches the scene is therefore
    deferred to the "load" event, which waywall fires once configuration
    loading has finished.
]]

local waywall = require("waywall")

local config = require("toolwall.config")
local commands = require("toolwall.commands")
local hud = require("toolwall.hud")
local keybinds = require("toolwall.keybinds")
local modes = require("toolwall.modes")
local launch = require("toolwall.launch")
local scene = require("toolwall.scene")
local state = require("toolwall.state")
local util = require("toolwall.util")

local M = {}

M.VERSION = "0.1.0"
M.SCHEMA_VERSION = 1

--[[
    Module-level state.

    This table MUST stay reachable for the lifetime of the Lua VM. waywall
    scene objects disappear when garbage collected, so anything we create and
    then drop the last reference to will silently vanish from the screen.
]]
local rt = {
    doc = nil,      -- parsed toolwall.json
    scene = nil,    -- scene registry (live mirror/image objects)
    modes = nil,    -- mode controller
    hud = nil,      -- hud controller
    degraded = nil, -- error string, if we fell back to last-known-good
}

M.rt = rt

--[[
    Build the waywall config table from the document.

    Only the parts waywall reads at load time: input, theme, window, shaders,
    experimental, actions. Scene content is not represented here.
]]
--[[
    waywall's own config parser only recognises "no anchor" as the Lua field
    being entirely absent (nil) - not an empty string. Our schema allows ""
    as the "none" choice for a GUI dropdown, so translate "", nil and JSON
    null all into an omitted key here, or every toolwall config that leaves
    ninb_anchor unset would fail to load at all:
    "invalid value '' for 'theme.ninb_anchor'".
]]
local function ninb_anchor_value(raw)
    if raw == nil or raw == util.NULL or raw == "" then
        return nil
    end
    return raw
end

local function build_waywall_config(doc)
    local input = doc.input or {}
    local theme = doc.theme or {}
    local window = doc.window or {}
    local experimental = doc.experimental or {}

    return {
        input = {
            layout = input.layout or "",
            model = input.model or "",
            rules = input.rules or "",
            variant = input.variant or "",
            options = input.options or "",

            remaps = input.remaps or {},

            repeat_rate = input.repeat_rate or -1,
            repeat_delay = input.repeat_delay or -1,

            sensitivity = input.sensitivity or 1.0,
            confine_pointer = util.bool(input.confine_pointer, false),
        },

        theme = {
            background = theme.background or "#000000ff",
            background_png = util.expand(theme.background_png) or "",

            cursor_theme = theme.cursor_theme or "",
            cursor_icon = theme.cursor_icon or "",
            cursor_size = theme.cursor_size or 0,

            ninb_anchor = ninb_anchor_value(theme.ninb_anchor),
            ninb_opacity = theme.ninb_opacity or 1.0,

            -- needs the toolwall patch. an unpatched waywall ignores unknown
            -- keys, so this is safe to send either way.
            ninb_hidden = util.bool(theme.ninb_hidden, false),
        },

        window = {
            fullscreen_width = window.fullscreen_width or 0,
            fullscreen_height = window.fullscreen_height or 0,
        },

        shaders = config.load_shaders(doc),

        experimental = {
            debug = util.bool(experimental.debug, false),
            jit = util.bool(experimental.jit, false),
            tearing = util.bool(experimental.tearing, false),
        },

        actions = {},  -- filled in below by keybinds.build
    }
end

--[[
    Choose the remap set for the instance's current screen.

    waywall.state() throws when the State Output mod is absent, so a config
    that opts in on an instance without the mod degrades to the base remaps
    rather than erroring on every state change.
]]
local function apply_state_remaps(doc)
    local input = doc.input or {}
    local base = input.remaps or {}
    local menu = input.remaps_menu or {}

    local ok, state = pcall(waywall.state)
    if not ok or type(state) ~= "table" then
        return
    end

    local playing = state.screen == "inworld" and state.inworld == "unpaused"
    waywall.set_remaps(playing and base or menu)
end

--[[
    Deferred initialisation. Runs on the "load" event, when the full waywall
    API is legal to call.
]]
local function on_load()
    rt.scene = scene.new(rt.doc)
    rt.modes = modes.new(rt.doc, rt.scene)
    rt.hud = hud.new(rt.doc, rt.modes)

    commands.bind(rt)

    --[[
        Restore the mode that was active before this VM was built.

        Every save reloads the config, which rebuilds the VM. Without this,
        editing a rectangle and watching it apply would drop you back to the
        base resolution on every keystroke, which makes live editing useless
        exactly when you want it most.
    ]]
    local resume = state.read("mode")
    local default = rt.doc.default_mode

    if resume and rt.doc._modes[resume] then
        rt.modes:set(resume)
    elseif default and default ~= util.NULL then
        rt.modes:set(default)
    else
        rt.modes:reset()
    end

    rt.hud:refresh()

    -- ninb is started from its own listener, below, so that it can wait.

    if next(rt.doc.input and rt.doc.input.remaps_menu or {}) then
        apply_state_remaps(rt.doc)
    end

    if rt.degraded then
        rt.hud:banner("toolwall: " .. rt.degraded .. " (using last known good)")
    end
end

--[[
    Entry point. Returns the table waywall expects from init.lua.

    opts:
      path , override the config path (default $XDG_CONFIG_HOME/waywall/toolwall.json)
]]
function M.setup(opts)
    opts = opts or {}

    local doc, err = config.load(opts.path)
    if not doc then
        -- A bad write from the GUI must never brick a running session.
        local fallback, fallback_err = config.load_last_good(opts.path)
        if not fallback then
            error(("toolwall: %s (and no last-known-good: %s)")
                :format(err, fallback_err or "none"))
        end
        doc = fallback
        rt.degraded = err
    else
        config.mark_good(opts.path)
    end

    rt.doc = doc

    local cfg = build_waywall_config(doc)
    cfg.actions = keybinds.build(doc, rt)

    waywall.listen("load", on_load)

    waywall.listen("resolution", function()
        if rt.hud then rt.hud:refresh() end
    end)

    --[[
        Swap remaps when the cursor appears.

        A bind that is useful with the cursor captured (a mouse button standing
        in for a key) is usually wrong once you are clicking around an
        inventory. waywall reports the instance's screen through the State
        Output mod, so "cursor visible" is everything that is not
        inworld/unpaused.

        Only wired up when a second set is actually configured, so the common
        case costs nothing and the State Output mod stays optional.
    ]]
    if next(doc.input and doc.input.remaps_menu or {}) then
        waywall.listen("state", function()
            apply_state_remaps(doc)
        end)
    end

    if doc.hud and doc.hud.follow_state then
        waywall.listen("state", function()
            if rt.hud then rt.hud:refresh() end
        end)
    end

    --[[
        Start Ninjabrain Bot, a little after everything else.

        THE PROBLEM

        ninb reads the X11 keymap once, when it starts, to build the table it
        translates key presses through. waywall brings its X server up *after*
        the config has run, so starting ninb from the config means starting it
        before there is an X server to read a keymap from. What it ends up with
        is the raw kernel keycodes, which are the X ones minus 8, so every key
        it sees is eight places out: right arrow reads as the numpad slash,
        left arrow as compose, and its hotkeys quietly stop working.

        It also wants to be first so it takes waywall's one anchor slot. Those
        two pull in opposite directions, and correctness wins: a late anchor
        sorts itself out, a wrong keymap does not.

        Its own listener, because the wait must not hold anything else up.
    ]]
    local ninb = doc.ninb or {}
    local ninb_cmd = launch.ninb_command(ninb)

    if util.bool(ninb.autostart, false) and ninb_cmd then
        waywall.listen("load", function()
            pcall(waywall.sleep, math.max(0, math.floor(ninb.start_delay_ms or 3000)))
            launch.once(waywall, "ninb", ninb_cmd)
        end)
    end

    --[[
        Start the ninb readout on its own, if it is enabled.

        It has to be its own listener: the loop only returns when the readout
        is turned off, and every listener gets its own coroutine, so anything
        sharing one with it would never run.
    ]]
    if util.bool((doc.ninb and doc.ninb.overlay or {}).enabled, false) then
        waywall.listen("load", function()
            commands.run_ninb_overlay(rt)
        end)
    end

    return cfg
end

return M
