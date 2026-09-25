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
    chat = false,   -- chat is open, because we saw the key that opens it
    trace = false,  -- log every state change, from experimental.debug
    keymap = nil,   -- the keymap waywall is on, so we never set the same one twice
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

    -- waywall is about to be on this one, so record it before anything asks
    -- to change it. Getting this wrong only costs a redundant set_keymap.
    rt.keymap = M.keymap_for(doc, false)

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
local function read_state()
    local ok, st = pcall(waywall.state)
    if not ok or type(st) ~= "table" then
        return nil
    end

    -- Back in the world with the cursor gone: whatever was open is closed.
    if st.screen == "inworld" and st.inworld == "unpaused" then
        rt.chat = false
    end

    return st
end

local function playing(st)
    return st.screen == "inworld" and st.inworld == "unpaused"
end

local function apply_state_remaps()
    if rt.typing then
        return
    end

    local st = read_state()
    if not st then
        return
    end

    if rt.trace then
        util.warn(("state %s/%s chat=%s"):format(
            tostring(st.screen), tostring(st.inworld), tostring(rt.chat)))
    end

    if playing(st) then
        waywall.set_remaps(rt.remaps.base)
    elseif rt.chat then
        --[[
            Not the menu set. The menu set is where searchcrafting lives, and
            a rebind that turns D into O is exactly as wrong in chat as it is
            right in a recipe search. Nothing at all is the only thing that
            reliably types what you pressed.
        ]]
        waywall.set_remaps({})
    else
        waywall.set_remaps(rt.remaps.menu)
    end
end

--[[
    The keymap, with and without whatever the config asked for.

    A search-crafting layout lives in the keymap, not in the rebinds, which is
    why toggling rebinds alone never fixed typing in chat: the letters were
    still coming out in the other language. faith raised exactly this in the
    waywall discord and nml's answer was a hand-written Lua snippet.

    Typing falls back to the layout the custom one is built on, which is what
    the keyboard did before toolwall touched it.

    Only a custom layout has anything to fall back from. Without one this
    returns the same table either way, and every caller below is written to
    do nothing when the keymap does not change, so chat mode costs a user who
    never built a layout precisely nothing.

    model, rules and options survive the swap: they describe the keyboard and
    the session, not the layout. variant does not - it names a variant of the
    custom layout, which the base does not have.
]]
function M.keymap_for(doc, typing)
    local input = doc.input or {}
    local custom = input.custom_layout

    -- A layout switched off in the editor keeps its keys in the file, so the
    -- block being there is not the same as it being in use.
    if type(custom) == "table" and util.bool(custom.enabled, true) == false then
        custom = nil
    end

    local km = {
        layout = input.layout or "",
        model = input.model or "",
        rules = input.rules or "",
        variant = input.variant or "",
        options = input.options or "",
    }

    if typing and type(custom) == "table" then
        km.layout = (type(custom.base) == "string" and custom.base ~= "") and custom.base or "us"
        km.variant = ""
    end

    return km
end

--[[
    Is there a keymap for chat mode to swap to?
]]
function M.has_chat_keymap(doc)
    local a, b = M.keymap_for(doc, false), M.keymap_for(doc, true)
    for _, key in ipairs({ "layout", "model", "rules", "variant", "options" }) do
        if a[key] ~= b[key] then
            return true
        end
    end
    return false
end

--[[
    Set the keymap, unless it is already the one we want.

    waywall's set_keymap is not free and not quiet. From use_local_keymap:

        seat->config->keymap = keymap;
        reset_keyboard_state(seat);

    and reset_keyboard_state sends a release for every key currently held.
    Setting the keymap you are already on would therefore drop the W you were
    walking with, for nothing. Every path that changes the keymap goes through
    here so that cannot happen, and so the automatic swap and the manual
    toggle cannot end up disagreeing about what is on.
]]
function M.apply_keymap(km)
    local live = rt.keymap
    if live
        and live.layout == km.layout
        and live.model == km.model
        and live.rules == km.rules
        and live.variant == km.variant
        and live.options == km.options
    then
        return true
    end

    local ok, err = pcall(waywall.set_keymap, km)
    if not ok then
        util.warn("could not change the keymap: " .. tostring(err))
        return false
    end

    rt.keymap = km
    return true
end

--[[
    Put the base layout back wherever typing happens.

    The other half of apply_state_remaps, and the half every config that tries
    this leaves out. Turning the rebinds off in a menu is no use if the
    letters are still coming out of a search-crafting layout: you get to chat
    with your rebinds gone and your keyboard still speaking Norwegian.

    Anything that is not inworld/unpaused counts, which is chat, inventories,
    signs, the pause menu and the title screen. The State Output mod cannot
    tell chat from a chest - both arrive as "gamescreenopen" - so this covers
    both, and the inventory does not care which layout it is on.

    A manual toggle outranks it, exactly as it does for the remaps.
]]
local function apply_state_keymap()
    if rt.typing then
        return
    end

    local st = read_state()
    if not st then
        return
    end

    --[[
        Chat, and anywhere outside a world: naming a world, an address, a
        search on the multiplayer screen.

        Deliberately NOT every in-world menu. A custom layout is usually there
        so you can searchcraft in it, and the recipe search is a menu, so
        turning the layout off in menus would take away the one place it was
        built for.
    ]]
    M.apply_keymap(M.keymap_for(rt.doc, rt.chat or st.screen ~= "inworld"))
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

        -- A sleep that fails means waywall is going away, and spinning the
        -- rest of the timeout doing nothing helps nobody.
        if not pcall(waywall.sleep, VIEW_POLL_MS) then
            return false
        end
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
    rt.hud = hud.new(rt.doc, rt.modes, rt)

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

    --[[
        Say so when the thing half these features need is not there.

        waywall.state() throws "no state output" when it has no instance, and
        every caller here pcalls it and moves on. That is right for the code
        and wrong for the person: menu rebinds and chat detection both quietly
        become no-ops, with nothing anywhere saying why. This is the one place
        that can tell the difference, so it says it once, at load.
    ]]
    local wants_state = next(rt.remaps.menu) ~= nil or M.has_chat_keymap(rt.doc)
    local state_ok = pcall(waywall.state)

    if wants_state and not state_ok then
        rt.degraded = rt.degraded
            or "no state output, so menu rebinds and chat mode do nothing"
        util.warn(
            "no state output from the instance. " ..
            "menu rebinds and chat mode need the State Output mod " ..
            "(worldpreview or state-output) in the Minecraft instance."
        )
    end

    if next(rt.remaps.menu) then
        apply_state_remaps()
    end

    if M.has_chat_keymap(rt.doc) then
        apply_state_keymap()
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

    --[[
        And swap the layout with it, for the same reason.

        Separate from the remaps listener because the two are configured
        separately: a custom layout with no menu remaps still wants this, and
        menu remaps with no custom layout have nothing to swap.
    ]]
    if M.has_chat_keymap(doc) then
        waywall.listen("state", apply_state_keymap)
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
    --[[
        Fullscreen on start, for people on a scaled desktop.

        waywall renders at the logical window size unless it is fullscreen and
        window.fullscreen_width is set, so on a 150% desktop the game is drawn
        at two thirds of the panel and stretched back up. That setting is the
        way out, and it only applies while fullscreen, so without this it does
        nothing until you remember to press a key.

        After the game window, because toggle_fullscreen is illegal during
        startup and there is nothing to make fullscreen before then anyway.
    ]]
    local window = doc.window or {}
    if util.bool(window.fullscreen_on_start, false) then
        waywall.listen("load", function()
            --[[
                Once per waywall, not once per reload. Every save reloads the
                config and runs this listener again, and toggle_fullscreen is
                a toggle: it went fullscreen on start, came back out on the
                next settings change, went back in on the one after. waywall
                exposes no way to ask whether it is fullscreen already, so the
                marker is the only way to know.
            ]]
            if not launch.first_time("fullscreen") then
                return
            end

            wait_for_game_window(60000)
            local ok, err = pcall(waywall.toggle_fullscreen)
            if not ok then
                util.warn("could not go fullscreen: " .. tostring(err))
            end
        end)
    end

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
            --[[
                The same wait the autostart does, and for a sharper reason.

                This loop forks a curl every 500ms until ninb answers. ninb
                does not even start until the game window exists, so before
                that every one of those forks is guaranteed waste, and they
                land exactly while waywall is bringing up Xwayland and
                Minecraft is connecting to it.

                Two sessions in ten died right there, both at the same line of
                waywall's log: "new connection from process N", then nothing.
                waywall goes down and takes the game with it. Whether the
                forks cause that or merely lose the race, there is no reason
                to be doing them at the one moment startup is most fragile.
            ]]
            wait_for_game_window(60000)
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
    rt.chat = false
    rt.trace = util.bool((doc.experimental or {}).debug, false)

    local cfg = build_waywall_config(doc)
    cfg.actions = keybinds.build(doc, rt)
    register_listeners(doc)

    return cfg
end

return M
