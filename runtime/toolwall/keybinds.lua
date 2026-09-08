--[[
    toolwall.keybinds — turn declarative keybind entries into waywall actions.

    The actions table must be built synchronously, before the "load" event,
    because waywall reads it from the returned config. The handlers themselves
    only run later, so they can safely close over runtime state that does not
    exist yet.
]]

local commands = require("toolwall.commands")
local util = require("toolwall.util")

local M = {}

--[[
    doc: parsed config document
    rt:  the runtime state table (populated on "load")
]]
function M.build(doc, rt)
    local actions = {}

    for _, bind in ipairs(doc.keybinds or {}) do
        local input = bind.input
        local command = bind.command
        local args = bind.args or {}

        if type(input) ~= "string" or input == "" then
            util.warn("keybind is missing an input")
        elseif type(command) ~= "string" or command == "" then
            util.warn(("keybind %q is missing a command"):format(input))
        elseif actions[input] then
            util.warn(("duplicate keybind for %q, ignoring"):format(input))
        else
            actions[input] = function()
                -- The scene does not exist until the load event has fired.
                if not rt.modes then
                    return false
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

    return actions
end

return M
