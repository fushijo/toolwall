--[[
    A config whose overlays are not tied to a resolution.

    helpers.res_mirror only shows at one resolution, which is what a toolwall
    mode is. waywall.mirror on its own is in the scene from load onwards and
    stays there, and the importer used to throw those away without a word.

    waywall refuses to create text during startup, so that one has to go in a
    load listener, which is also where plenty of configs put their mirrors.
]]

local waywall = require("waywall")
local helpers = require("waywall.helpers")

waywall.mirror({
    src = { x = 0, y = 0, w = 60, h = 60 },
    dst = { x = 10, y = 10, w = 120, h = 120 },
    depth = 2,
})

waywall.listen("load", function()
    waywall.mirror({
        src = { x = 100, y = 0, w = 40, h = 40 },
        dst = { x = 200, y = 10, w = 80, h = 80 },
    })
    waywall.image("resources/thing.png", { dst = { x = 0, y = 300, w = 50, h = 50 } })
    waywall.text("hello", { x = 5, y = 5, size = 2, color = "#ffffff" })
end)

return {
    actions = {
        -- One mode, so the always-on ones have something to not belong to.
        ["Shift-T"] = helpers.toggle_res(400, 16384),
    },
}
