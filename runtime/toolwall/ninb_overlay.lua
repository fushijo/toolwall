--[[
    toolwall.ninb_overlay, draw ninb's readout into the scene.

    waywall's text primitive takes x, y, colour, size and depth. there is no
    rectangle, no border, no outline and no font choice. so:

      - every row is its own text object, stacked by row_spacing
      - the panel behind them is a solid png stretched to the rect and
        recoloured with a colour key, which is the only way to get a filled
        area out of a compositor with no rect primitive
      - the border is a second, slightly larger panel drawn behind the first

    text objects have no setter, so redrawing means closing and recreating.
]]

local waywall = require("waywall")

local api = require("toolwall.ninb_api")
local util = require("toolwall.util")

local M = {}

local Overlay = {}
Overlay.__index = Overlay

local PANEL = "~/.config/waywall/resources/panel.png"

function M.new(cfg)
    return setmetatable({
        cfg = cfg or {},
        objects = {},
        last_line = nil,
        last_change = 0,
    }, Overlay)
end

function Overlay:clear()
    for _, obj in ipairs(self.objects) do
        pcall(function() obj:close() end)
    end
    self.objects = {}
end

local function opt(cfg, key, fallback)
    local value = cfg[key]
    if value == nil or value == util.NULL then return fallback end
    return value
end

--[[
    a filled rectangle, faked from a solid png plus a colour key.
]]
function Overlay:_panel(rect, colour, depth)
    local ok, obj = pcall(waywall.image, util.expand(PANEL), {
        dst = rect,
        depth = depth,
        color_key = { input = "#ffffff", output = colour },
    })
    if ok and obj then
        table.insert(self.objects, obj)
    end
end

function Overlay:_text(line, x, y, colour, size, depth)
    if line == nil or line == "" then return end

    local ok, obj = pcall(waywall.text, line, {
        x = x, y = y, color = colour, size = size, depth = depth,
    })
    if ok and obj then
        table.insert(self.objects, obj)
    end
end

--[[
    colour a certainty by how good it is, the way every other tool does.
]]
function Overlay:_certainty_colour(fields)
    local cfg = self.cfg
    local pct = tonumber(tostring(fields.certainty or ""):match("[%d%.]+"))
    if not pct then return opt(cfg, "color", "#ffffffff") end

    if pct >= opt(cfg, "certainty_high_above", 80) then
        return opt(cfg, "certainty_high_color", "#55ff55ff")
    elseif pct >= opt(cfg, "certainty_mid_above", 50) then
        return opt(cfg, "certainty_mid_color", "#ffaa00ff")
    end
    return opt(cfg, "certainty_low_color", "#ff5555ff")
end

--[[
    build the rows to draw from a stronghold response.

    returns a list of { text, colour }. empty means nothing to show, which is
    not the same as an error.
]]
function Overlay:rows(data)
    local cfg = self.cfg
    local rows = {}

    local predictions = type(data) == "table" and data.predictions or nil
    local count = math.max(1, math.floor(opt(cfg, "shown_predictions", 1)))
    local template = opt(cfg, "template", "{chunkX}, {chunkZ}  {certainty}")
    local chunk_coords = opt(cfg, "coords", "chunk") == "chunk"

    if type(predictions) == "table" and #predictions > 0 then
        for i = 1, math.min(count, #predictions) do
            local fields = api.prediction_fields(predictions[i], data, chunk_coords)
            if fields then
                table.insert(rows, {
                    text = api.render(template, fields),
                    colour = self:_certainty_colour(fields),
                })
            end
        end
    end

    if #rows == 0 then
        local idle = opt(cfg, "idle_text", "ninb: no throws")
        if idle ~= "" then
            table.insert(rows, { text = idle, colour = opt(cfg, "header_color", "#aaaaaaff") })
        end
    end

    -- eye throws listed underneath, oldest first
    local throw_rows = math.floor(opt(cfg, "throw_rows", 0))
    local throws = type(data) == "table" and data.eyeThrows or nil
    if throw_rows > 0 and type(throws) == "table" then
        for i = 1, math.min(throw_rows, #throws) do
            local t = throws[i]
            if type(t) == "table" then
                table.insert(rows, {
                    text = ("throw %d  %s, %s"):format(
                        i, api.number(t.x), api.number(t.z)),
                    colour = opt(cfg, "header_color", "#aaaaaaff"),
                })
            end
        end
    end

    return rows
end

--[[
    redraw everything. cheap enough at poll rates, and the only option given
    text objects cannot be mutated.
]]
function Overlay:draw(data, now)
    local cfg = self.cfg
    self:clear()

    local rows = self:rows(data)
    if #rows == 0 then return end

    local joined = ""
    for _, row in ipairs(rows) do joined = joined .. row.text .. "\n" end

    -- stale handling: drop the panel when nothing has changed for a while
    if joined ~= self.last_line then
        self.last_line = joined
        self.last_change = now or 0
    end

    local hide_after = math.floor(opt(cfg, "hide_after_ms", 0))
    if hide_after > 0 and now and (now - self.last_change) > hide_after then
        return
    end

    local x = math.floor(opt(cfg, "x", 8))
    local y = math.floor(opt(cfg, "y", 40))
    local size = math.max(1, math.floor(opt(cfg, "size", 2)))
    local spacing = math.max(1, math.floor(opt(cfg, "row_spacing", 18)))
    local pad = math.floor(opt(cfg, "padding", 6))

    if util.bool(opt(cfg, "background", false), false) then
        -- 6px per character at size 1 is waywall's bundled face, near enough
        -- to size a panel that has to be guessed rather than measured
        local widest = 0
        for _, row in ipairs(rows) do
            widest = math.max(widest, #row.text)
        end

        local w = widest * 6 * size + pad * 2
        local h = #rows * spacing + pad * 2

        local border = math.floor(opt(cfg, "border_width", 0))
        if border > 0 then
            self:_panel({
                x = x - pad - border, y = y - pad - border,
                w = w + border * 2, h = h + border * 2,
            }, opt(cfg, "border_color", "#aaaaaaff"), 7)
        end

        self:_panel({ x = x - pad, y = y - pad, w = w, h = h },
            opt(cfg, "background_color", "#000000b0"), 8)
    end

    for index, row in ipairs(rows) do
        self:_text(row.text, x, y + (index - 1) * spacing, row.colour, size, 9)
    end
end

return M
