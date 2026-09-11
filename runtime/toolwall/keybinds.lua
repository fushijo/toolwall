--[[
    toolwall.keybinds, turn declarative keybind entries into waywall actions.

    The actions table must be built synchronously, before the "load" event,
    because waywall reads it from the returned config. The handlers themselves
    only run later, so they can safely close over runtime state that does not
    exist yet.
]]

local waywall = require("waywall")

local commands = require("toolwall.commands")
local util = require("toolwall.util")

local M = {}

--[[
    The one command that survives a suspension, because it is the way back.

    Suspending every key including this one would leave no way to unsuspend
    from inside the game, and the editor is where the switch lives.
]]
local ESCAPE_COMMAND = "gui.toggle"

--[[
    doc: parsed config document
    rt:  the runtime state table (populated on "load")
]]
function M.build(doc, rt)
    local actions = {}

    local suspended = util.bool(doc.suspend_keybinds, false)
    local escape = nil

    for _, bind in ipairs(doc.keybinds or {}) do
        local input = bind.input
        local command = bind.command
        local args = bind.args or {}
        local f3_safe = util.bool(bind.f3_safe, true)

        if suspended and command ~= ESCAPE_COMMAND then
            -- left unbound, so the key reaches Minecraft untouched
        elseif type(input) ~= "string" or input == "" then
            util.warn("keybind is missing an input")
        elseif type(command) ~= "string" or command == "" then
            util.warn(("keybind %q is missing a command"):format(input))
        elseif actions[input] then
            util.warn(("duplicate keybind for %q, ignoring"):format(input))
        else
            if command == ESCAPE_COMMAND then
                escape = input
            end

            actions[input] = function()
                -- The scene does not exist until the load event has fired.
                if not rt.modes then
                    return false
                end

                --[[
                    Do not fire while F3 is held.

                    waywall matches modifiers exactly, so a bind on "B" already
                    ignores Shift, Ctrl and Alt. F3 is an ordinary key rather
                    than a modifier, so nothing stops F3+B from also triggering
                    a bind on B, and reaching for hitboxes would flip you into
                    thin. Returning false passes the key through to Minecraft,
                    so the F3 combo does what it should.
                ]]
                if f3_safe then
                    local held, pressed = pcall(waywall.get_key, "F3")
                    if held and pressed then
                        return false
                    end
                end

                local fn = commands.get(command)
                if not fn then
                    util.warn(("unknown command %q"):format(command))
                    return false
                end

                local ok, result = pcall(fn, rt, args)
                if not ok then
                    util.warn(("command %q failed: %s"):format(command, tostring(result)))
                    return false
                end

                -- Explicit false means "pass the input through to Minecraft".
                if result == false then
                    return false
                end
            end
        end
    end

    if suspended then
        if escape then
            util.warn(("keybinds suspended, %s still opens the editor"):format(escape))
        else
            util.warn("keybinds suspended, and nothing here opens the editor. " ..
                "run: toolwall set suspend_keybinds false")
        end
    end

    return actions
end

return M
