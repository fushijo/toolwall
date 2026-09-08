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
        Floating-window control.

        KNOWN LIMITATION: waywall.show_floating() is global. It shows or hides
        every floating window at once, including Ninjabrain Bot. There is
        currently no per-window control and no anchoring for anything other
        than Ninjabrain Bot (theme.ninb_anchor is hardcoded to it).

        Generalising this is the first upstream patch worth writing. Until
        then, opening the toolwall GUI will also reveal Ninjabrain Bot.
    ]]
    M.register("floating.toggle", function()
        waywall.show_floating(not waywall.floating_shown())
    end)

    M.register("floating.show", function()
        waywall.show_floating(true)
    end)

    M.register("floating.hide", function()
        waywall.show_floating(false)
    end)

    M.register("gui.toggle", function(state)
        local gui = state.doc.gui or {}

        if not state.gui_launched then
            local cmd = gui.command
            if not cmd or cmd == util.NULL or cmd == "" then
                util.warn("no gui.command configured")
                return false
            end
            waywall.exec(cmd)
            state.gui_launched = true

            -- Give the client a moment to map before revealing it.
            waywall.sleep(gui.launch_delay_ms or 400)
        end

        waywall.show_floating(not waywall.floating_shown())
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
