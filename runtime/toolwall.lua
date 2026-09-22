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
local keycodes = require("toolwall.keycodes")
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
-- the shoebox. everything in here has to stay reachable or the garbage
-- collector quietly eats your overlays.
local rt = {
    doc = nil,      -- parsed toolwall.json
    remaps = nil,   -- { base, menu }, filtered once at setup
    typing = false, -- chat mode, toggled by hand, reset by a reload
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

            remaps = rt.remaps.base,

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
local function apply_state_remaps()
    --[[
        A manual chat toggle outranks the automatic one.

        Without this, opening a chest while typing would quietly put the game
        rebinds back and you would be searching for a stronghold with the T
        key producing a 4.
    ]]
    if rt.typing then
        return
    end

    local ok, state = pcall(waywall.state)
    if not ok or type(state) ~= "table" then
        return
    end

    local playing = state.screen == "inworld" and state.inworld == "unpaused"
    waywall.set_remaps(playing and rt.remaps.base or rt.remaps.menu)
end

--[[
    The keymap, with and without whatever the config asked for.

    A search-crafting layout lives in the keymap, not in the rebinds, which is
    why toggling rebinds alone never fixed typing in chat: the letters were
    still coming out in the other language. faith raised exactly this in the
    waywall discord and nml's answer was a hand-written Lua snippet.

    Typing falls back to the layout the custom one is built on, which is what
    the keyboard did before toolwall touched it.
]]
function M.keymap_for(doc, typing)
    local input = doc.input or {}
    local custom = input.custom_layout

    if typing then
        local base = "us"
        if type(custom) == "table" and type(custom.base) == "string" and custom.base ~= "" then
            base = custom.base
        elseif type(input.layout) == "string" and input.layout ~= "" then
            base = input.layout
        end
        return { layout = base, model = "", rules = "", variant = "", options = "" }
    end

    return {
        layout = input.layout or "",
        model = input.model or "",
        rules = input.rules or "",
        variant = input.variant or "",
        options = input.options or "",
    }
end

--[[
    Block until Minecraft's window exists.

    waywall treats the first view it sees as the game. From on_view_create:

        if (strcmp(view->impl->name, "xwayland") == 0) {
            ... "X11 minecraft detected" ...
            kill(pid, SIGKILL);
        }

    Ninjabrain Bot is a Java app, so its window is Xwayland. Start it before
    Minecraft has mapped its own surface and waywall decides ninb *is* the
    game running under X11, prints a banner telling you to check your GLFW
    path, and kills it. Nothing is wrong with the GLFW path; ninb simply got
    there first.

    That is what a fixed delay cannot fix. Three seconds is longer than
    Minecraft usually takes, so it mostly works, and then a cold cache or a
    slow mod load puts the game over the line and ninb dies. Occasional by
    construction.

    The probe: set_resolution errors while wrap->view is null, and returns
    early without touching anything when the requested resolution is the
    active one already. Asking for 0x0 while 0x0 is active is therefore a
    pure question, and the answer is whether the window is there.
]]
-- are we there yet. are we there yet. are we there yet.
local VIEW_POLL_MS = 100

local function game_window_exists()
    local ok, w = pcall(waywall.active_res)
    if ok and type(w) == "number" and w > 0 then
        -- A live resolution can only have been set through a view.
        return true
    end

    return (pcall(waywall.set_resolution, 0, 0))
end

local function wait_for_game_window(timeout_ms)
    local waited = 0

    while waited < timeout_ms do
        if game_window_exists() then
            return true
        end

        pcall(waywall.sleep, VIEW_POLL_MS)
        waited = waited + VIEW_POLL_MS
    end

    return false
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

    if next(rt.remaps.menu) then
        apply_state_remaps()
    end

    if rt.degraded then
        rt.hud:banner("toolwall: " .. rt.degraded .. " (using last known good)")
    end
end

--[[
    Wire up everything that reacts to waywall rather than to a keypress.

    Each listener runs in its own coroutine, which is what lets the two that
    wait, ninb's start and the readout's loop, wait without holding up the
    others or the rest of setup.
]]
local function register_listeners(doc)
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
    if next(rt.remaps.menu) then
        waywall.listen("state", apply_state_remaps)
    end

    if doc.hud and doc.hud.follow_state then
        waywall.listen("state", function()
            if rt.hud then rt.hud:refresh() end
        end)
    end

    --[[
        Start Ninjabrain Bot, once there is a game for it to sit on top of.

        TWO THINGS IT MUST NOT BE EARLIER THAN

        ninb reads the X11 keymap once, at startup, to build the table it
        translates key presses through. waywall brings its X server up *after*
        the config runs, so starting ninb straight from the config leaves it
        with raw kernel keycodes, which are the X ones minus 8: right arrow
        reads as the numpad slash, left arrow as compose, and its hotkeys
        quietly stop working.

        And waywall takes the first view it sees as the game. ninb's window is
        Xwayland, so arriving first gets it killed with a banner about X11
        Minecraft. See wait_for_game_window above.

        Waiting for the window settles both, because Minecraft cannot map a
        surface before the X server exists. start_delay_ms is now grace on top
        of that rather than a guess at how long the game takes, so it no longer
        has a race to lose.

        The anchor still works out: waywall's one anchor slot is for floating
        windows, and the game is not one, so ninb is still first in that queue.

        Its own listener, because the wait must not hold anything else up.
    ]]
    local ninb = doc.ninb or {}
    local ninb_cmd = launch.ninb_command(ninb)

    if util.bool(ninb.autostart, false) and ninb_cmd then
        waywall.listen("load", function()
            --[[
                Capped, so an instance that never comes up does not mean an
                ninb that never starts. Past the cap we are no worse off than
                the blind sleep this replaced.
            ]]
            if not wait_for_game_window(60000) then
                util.warn("ninb: no game window after 60s, starting it anyway")
            end

            pcall(waywall.sleep, math.max(0, math.floor(ninb.start_delay_ms or 500)))
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

    --[[
        Filter the rebinds once, here, rather than wherever they are used.

        The warnings belong to loading a config, not to playing with one: the
        state listener runs on every screen change, and sanitising there would
        reprint the same complaint every time you opened a chest.
    ]]
    local input = doc.input or {}
    rt.remaps = {
        base = keycodes.sane(input.remaps, "remaps"),
        menu = keycodes.sane(input.remaps_menu, "remaps_menu"),
    }

    -- A reload rebuilds the Lua VM but this table outlives it, so chat mode
    -- would otherwise survive an edit and leave you wondering why your keys
    -- stopped working.
    rt.typing = false

    local cfg = build_waywall_config(doc)
    cfg.actions = keybinds.build(doc, rt)
    register_listeners(doc)

    return cfg
end

return M
