--[[
    A hand-written waywall config, for the importer tests.

    Deliberately not gore's: it uses the toggle_res helper instead of building
    resolution closures, writes its actions table inline, and carries one
    rebind that waywall would refuse. The importer has to read it by what it
    does, not by recognising a layout it has seen before.
]]

local waywall = require("waywall")
local helpers = require("waywall.helpers")

helpers.res_mirror({
    src = { x = 0, y = 0, w = 60, h = 580 },
    dst = { x = 30, y = 340, w = 600, h = 400 },
    depth = 2,
}, 400, 16384)

return {
    input = {
        sensitivity = 2.5,
        repeat_rate = 20,
        remaps = {
            ["MB4"] = "HOME",
            ["P"] = "Escape",   -- keysym, not a keycode: waywall refuses this
            ["X"] = "",         -- half-filled
        },
    },
    theme = {
        background = "#101010",
    },
    window = {
        fullscreen_width = 1920,
        fullscreen_height = 1080,
    },
    actions = {
        ["Shift-T"] = helpers.toggle_res(400, 16384),
        ["Shift-W"] = helpers.toggle_res(1920, 280),
        ["Shift-F"] = waywall.toggle_fullscreen,
        ["Shift-N"] = function()
            waywall.exec("java -jar /opt/ninb/Ninjabrain-Bot-1.5.2.jar")
        end,
        ["Shift-Z"] = function()
            -- nothing toolwall has a verb for
            local _ = waywall.floating_shown()
        end,
    },
}
