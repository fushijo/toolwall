--[[
    toolwall.commands — the closed set of verbs a keybind may invoke.

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

        KNOWN LIMITATION: waywall.show_floating() is global — it shows or hides
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
            otherwise lose the fact that the GUI is already open — so the
            next press would open a second one, and the next a third.
        ]]
        if launch.once(waywall, "gui", cmd) then
            local slept = pcall(waywall.sleep, gui.launch_delay_ms or 400)
            if not slept then
                util.warn("launch delay failed; revealing the GUI anyway")
            end

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
        Launch a configured app as a floating window, then reveal it.

        This is what makes a "toggle Ninjabrain Bot" keybind actually open
        Ninjabrain Bot. floating.toggle only changes the visibility of windows
        that are already running, so on its own it can never start anything.

        KNOWN LIMITATION: waywall.show_floating() is global, so revealing one
        floating window reveals every running one. Showing only the app you
        asked for needs per-window control, which is an upstream change.
    ]]
    M.register("app.toggle", function(state, args)
        local id = args.app
        local app = state.doc._apps and state.doc._apps[id]
        if not app then
            util.warn(("unknown app %q"):format(tostring(id)))
            return false
        end

        local cmd = app.command
        if not cmd or cmd == util.NULL or cmd == "" then
            util.warn(("app %q has no command"):format(id))
            return false
        end

        cmd = util.expand_command(cmd)

        -- Tracked by pidfile for the same reason gui.toggle is: a reload
        -- must not start a second Ninjabrain Bot.
        if launch.once(waywall, "app-" .. id, cmd) then
            local gui = state.doc.gui or {}
            pcall(waywall.sleep, gui.launch_delay_ms or 400)

            force_show_floating()
            return
        end

        if waywall.floating_shown() then
            waywall.show_floating(false)
        else
            force_show_floating()
        end
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
