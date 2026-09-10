--[[
    toolwall.commands, the closed set of verbs a keybind may invoke.

    Keybinds in toolwall.json name a command and pass arguments. They never
    contain Lua source. That is the whole point: a GUI can safely author a
    keybind table, and a config downloaded from someone else cannot execute
    arbitrary code on load.

    The one exception is "exec", which is opt-in and off by default. waywall's
    own documentation warns that Lua config code can spawn subprocesses and
    write files; toolwall does not widen that hole implicitly.
]]

local waywall = require("waywall")
local launch = require("toolwall.launch")
local ninb_api = require("toolwall.ninb_api")
local util = require("toolwall.util")

local M = {}

-- name -> function(rt, args) -> boolean|nil
-- Returning false propagates to waywall as "input not consumed".
M.registry = {}

function M.register(name, fn)
    M.registry[name] = fn
end

function M.get(name)
    return M.registry[name]
end

--[[
    Bind the built-in commands. Called once the runtime state is populated.
]]
function M.bind(rt)
    M.register("mode.set", function(state, args)
        return state.modes:set(args.mode)
    end)

    M.register("mode.reset", function(state)
        return state.modes:reset()
    end)

    M.register("mode.cycle", function(state, args)
        return state.modes:cycle(args.modes)
    end)

    M.register("sens.set", function(_, args)
        waywall.set_sensitivity(args.sensitivity or 0)
    end)

    M.register("keymap.set", function(_, args)
        waywall.set_keymap({
            layout = args.layout,
            model = args.model,
            rules = args.rules,
            variant = args.variant,
            options = args.options,
        })
    end)

    M.register("remaps.set", function(_, args)
        waywall.set_remaps(args.remaps or {})
    end)

    M.register("key.press", function(_, args)
        waywall.press_key(args.key)
    end)

    M.register("fullscreen.toggle", function()
        waywall.toggle_fullscreen()
    end)

    --[[
        Force every floating window to be shown, and committed.

        KNOWN LIMITATION: waywall.show_floating() is global, it shows or hides
        every floating window at once, so revealing the GUI also reveals
        Ninjabrain Bot. Per-window control is an upstream change.

        waywall's floating_set_visible() returns early when the requested
        visibility already matches the current flag:

            if (wrap->floating.visible == visible) return;

        and it is that function - not the per-view creation path - which
        commits the views:

            wl_list_for_each (fview, &wrap->floating.views, link) {
                server_view_set_visible(fview->view, visible);
                server_view_commit(fview->view);
            }

        A window that maps while the flag is already true therefore gets
        set_visible() at creation with no commit, and a later show(true) is a
        no-op because the flag already matches. The window is then visible
        according to waywall and absent from the screen, permanently.

        Driving the flag false and back to true forces the transition, so the
        commit loop runs over every live floating view.
    ]]
    local function force_show_floating()
        if waywall.floating_shown() then
            waywall.show_floating(false)
        end
        waywall.show_floating(true)
    end

    M.register("floating.toggle", function()
        if waywall.floating_shown() then
            waywall.show_floating(false)
        else
            force_show_floating()
        end
    end)

    M.register("floating.show", function()
        force_show_floating()
    end)

    M.register("floating.hide", function()
        waywall.show_floating(false)
    end)

    --[[
        Free the cursor so the editor can actually be clicked.

        waywall routes a click to a floating window only when Minecraft does
        not hold the pointer:

            if (wrap->input.pointer_locked) return false;   // wrap.c

        and it exposes no way to release that lock. The one lever available is
        the same one a player uses - press Escape into the game, which opens
        the pause menu and hands the cursor back.

        The keycode is "ESC", from input-event-codes.h. waywall takes keycodes
        here and keysyms in keybinds, and passing a keysym name ("Escape")
        fails silently - which is exactly why this did nothing at first.

        Guarded on the instance actually being in-world and unpaused, so this
        never closes a menu that is already open. Without the State Output mod
        we cannot tell, and do nothing rather than guess.
    ]]
    local function release_cursor()
        local ok, st = pcall(waywall.state)
        if not ok or type(st) ~= "table" then
            return
        end

        if st.screen == "inworld" and st.inworld == "unpaused" then
            pcall(waywall.press_key, "ESC")
        end
    end

    M.register("gui.toggle", function(state)
        local gui = state.doc.gui or {}
        local cmd = gui.command

        if not cmd or cmd == util.NULL or cmd == "" then
            util.warn("no gui.command configured")
            return false
        end

        cmd = util.expand_command(cmd)

        --[[
            Launch state lives in a pidfile, not in this table. Saving from
            the GUI reloads the config, which rebuilds the Lua VM and would
            otherwise lose the fact that the GUI is already open, so the
            next press would open a second one, and the next a third.
        ]]
        if launch.once(waywall, "gui", cmd) then
            local slept = pcall(waywall.sleep, gui.launch_delay_ms or 400)
            if not slept then
                util.warn("launch delay failed; revealing the GUI anyway")
            end

            release_cursor()
            force_show_floating()
            return
        end

        if waywall.floating_shown() then
            waywall.show_floating(false)
        else
            release_cursor()
            force_show_floating()
        end
    end)

    --[[
        Open Ninjabrain Bot, launching it first if it is not already running.

        floating.toggle only changes the visibility of windows that are
        already open, so on its own it can never start ninb - which is why a
        key bound to it appeared to do nothing.

        KNOWN LIMITATION: show_floating() is global, so this reveals every
        floating window, the editor included. Per-window control is upstream.
    ]]
    M.register("ninb.toggle", function(st)
        local ninb = st.doc.ninb or {}
        local jar = ninb.jar

        if not jar or jar == util.NULL or jar == "" then
            util.warn("no Ninjabrain Bot jar configured")
            return false
        end

        local template = ninb.command
        if not template or template == util.NULL or template == "" then
            template = "java -jar {jar}"
        end

        local cmd = util.expand_command((template:gsub("{jar}", util.expand(jar))))

        if launch.once(waywall, "ninb", cmd) then
            pcall(waywall.sleep, (st.doc.gui or {}).launch_delay_ms or 400)
            force_show_floating()
            return
        end

        if waywall.floating_shown() then
            waywall.show_floating(false)
        else
            force_show_floating()
        end
    end)

    --[[
        ninb's stronghold readout, drawn as scene text.

        runs as a loop inside the keybind's coroutine: waywall.sleep yields, so
        other handlers keep running while this waits. the loop dies with the
        lua vm, so a config reload stops it without any cleanup.

        the fetch is fire-and-forget, so each pass draws what the previous pass
        fetched. one poll of lag, invisible at this refresh rate.
    ]]
    M.register("ninb.overlay", function(st)
        if st.ninb_overlay then
            st.ninb_overlay = false
            if st.ninb_text then
                st.ninb_text:close()
                st.ninb_text = nil
            end
            util.warn("ninb overlay off")
            return
        end

        local ninb = st.doc.ninb or {}
        local overlay = ninb.overlay or {}
        local port = ninb.port or ninb_api.DEFAULT_PORT
        local poll = overlay.poll_ms or 500
        local template = overlay.template or "{chunkX}, {chunkZ}  {certainty}"

        st.ninb_overlay = true
        util.warn(("ninb overlay on, polling :%d every %dms"):format(port, poll))

        while st.ninb_overlay do
            ninb_api.fetch("stronghold", port)

            -- idle text keeps the overlay visible before the first throw, so
            -- you can see it is alive and where it sits
            local fields = ninb_api.stronghold_fields(ninb_api.read("stronghold"))
            local line = fields and ninb_api.render(template, fields)
                or (overlay.idle_text or "ninb: no throws")

            -- text has no setter, so redrawing means replacing the object
            if st.ninb_text then
                st.ninb_text:close()
                st.ninb_text = nil
            end

            if line ~= "" then
                local ok, obj = pcall(waywall.text, line, {
                    x = overlay.x or 8,
                    y = overlay.y or 40,
                    color = overlay.color or "#ffffffff",
                    size = overlay.size or 2,
                    depth = 10,
                })
                if ok then st.ninb_text = obj end
            end

            local slept = pcall(waywall.sleep, poll)
            if not slept then
                util.warn("ninb overlay: sleep failed, stopping")
                st.ninb_overlay = false
            end
        end
    end)

    --[[
        Show or hide one overlay by hand, independent of the current mode.

        Modes own the overlays they declare; this pins one on top of whatever
        mode is active, and it survives mode switches until toggled off.
    ]]
    M.register("overlay.toggle", function(state, args)
        return state.scene:toggle(args.overlay)
    end)

    M.register("exec", function(state, args)
        local gui = state.doc.gui or {}
        if not util.bool(gui.allow_exec, false) then
            util.warn("exec command is disabled (set gui.allow_exec)")
            return false
        end
        waywall.exec(args.command)
    end)
end

return M
