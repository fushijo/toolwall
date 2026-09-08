--[[
    toolwall — a data-driven configuration layer for waywall.

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
local scene = require("toolwall.scene")
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

    -- Apply the default mode, if one is configured.
    local default = rt.doc.default_mode
    if default and default ~= util.NULL then
        rt.modes:set(default)
    else
        rt.modes:reset()
    end

    rt.hud:refresh()

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
      path  — override the config path (default $XDG_CONFIG_HOME/waywall/toolwall.json)
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

    return cfg
end

return M
