--[[
    A config that has already taken the key toolwall wants for the editor.

    "*-I" carries the modifier wildcard, so it fires on Ctrl-I as well and the
    importer has to go and find another key. The second bind is wrapped in
    ingame_only, which the importer can only spot by running it twice.
]]

local waywall = require("waywall")
local helpers = require("waywall.helpers")

return {
    actions = {
        ["*-I"] = helpers.toggle_res(400, 16384),
        ["Shift-B"] = helpers.ingame_only(helpers.toggle_res(1920, 280)),
        -- Not ninjabrain, so it lands on the generic exec path.
        ["Shift-P"] = function()
            waywall.exec("java -jar /opt/paceman/paceman-tracker.jar --nogui")
        end,
    },
}
