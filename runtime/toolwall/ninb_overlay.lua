--[[
    toolwall.ninb_overlay, draw ninb's readout into the scene.

    WHY THIS EXISTS

    Ninjabrain Bot's own window is a floating window, and waywall's
    show_floating() is global. So ninb appears and vanishes together with every
    other floating window, including this project's editor. Text drawn into the
    scene is not a window, so it stays put.

    LAYOUT

    waywall's face is Terminus at a fixed 8x16 per character, scaled by an
    integer multiplier. Fixed width means a column is just a character count,
    so labels and values line up exactly and the panel behind them can be
    measured rather than guessed.

    A row is a list of cells, each with its own colour and starting column.
    That is what gives grey labels next to coloured values, and the eye throw
    table its columns, out of a text primitive that only takes one colour.

    PANEL

    waywall.rect() is the toolwall patch (see patches/ in the repo). Without it
    there is no way to fill an area: images cannot be colour keyed upstream, so
    a PNG can only ever be drawn as itself. The background is skipped rather
    than faked when the patch is missing.
]]

local waywall = require("waywall")

local api = require("toolwall.ninb_api")
local util = require("toolwall.util")

local M = {}

local Overlay = {}
Overlay.__index = Overlay

-- waywall's bundled terminus face, before the size multiplier
local CHAR_W = 8
local CHAR_H = 16

-- eye throw table, as character columns
local THROW_COLS = { 1, 10, 19, 28 }

function M.new(cfg)
    return setmetatable({
        cfg = cfg or {},
        objects = {},
        last_line = nil,
        last_change = 0,
        warned = false,
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

local function cell(text, colour, col)
    return { text = tostring(text or ""), colour = colour, col = col or 0 }
end

--[[
    colour a certainty by how good it is, the way every other tool does.
]]
function Overlay:_certainty_colour(fields)
    local cfg = self.cfg
    local pct = tonumber(fields.certaintyValue)
    if not pct then
        pct = tonumber(tostring(fields.certainty or ""):match("[%d%.]+"))
    end
    if not pct then return opt(cfg, "color", "#ffffffff") end

    if pct >= opt(cfg, "certainty_high_above", 80) then
        return opt(cfg, "certainty_high_color", "#55ff55ff")
    elseif pct >= opt(cfg, "certainty_mid_above", 50) then
        return opt(cfg, "certainty_mid_color", "#ffaa00ff")
    end
    return opt(cfg, "certainty_low_color", "#ff5555ff")
end

--[[
    break a long message onto several lines at word boundaries.
]]
local function wrap(text, width)
    local lines, line = {}, ""

    for word in tostring(text):gmatch("%S+") do
        if line == "" then
            line = word
        elseif #line + 1 + #word <= width then
            line = line .. " " .. word
        else
            table.insert(lines, line)
            line = word
        end
    end

    if line ~= "" then table.insert(lines, line) end
    return lines
end

--[[
    the ninb window, rebuilt: a label column and a value column, then the
    information messages, then the eye throws.
]]
function Overlay:_ninbot_rows(fields, data, messages)
    local cfg = self.cfg
    local rows = {}

    local label_colour = opt(cfg, "label_color", "#aaaaaaff")
    local value_colour = opt(cfg, "color", "#ffffffff")
    local coords = opt(cfg, "coords", "block")

    local pairs_out = {}
    local function add(label, text, colour)
        if text and text ~= "" then
            table.insert(pairs_out, { label = label, text = text, colour = colour })
        end
    end

    if util.bool(opt(cfg, "show_location", true), true) then
        local where = ("(%s, %s)"):format(fields.x or "?", fields.z or "?")
        if fields.distance and util.bool(opt(cfg, "show_distance", true), true) then
            where = where .. (", %s blocks away"):format(fields.distance)
        end
        add(coords == "chunk" and "Chunk:" or "Location:", where, value_colour)
    end

    if util.bool(opt(cfg, "show_certainty", true), true) then
        add("Certainty:", fields.certainty, self:_certainty_colour(fields))
    end

    -- the row the overworld-only tools leave out. a run reaches the stronghold
    -- through the nether far more often than by walking.
    if util.bool(opt(cfg, "show_nether", true), true) and fields.netherX then
        local where = ("(%s, %s)"):format(fields.netherX, fields.netherZ)
        if fields.netherDistance and util.bool(opt(cfg, "show_distance", true), true) then
            where = where .. (", %s blocks away"):format(fields.netherDistance)
        end
        add("Nether coords:", where, value_colour)
    end

    if util.bool(opt(cfg, "show_angle", false), false) then
        -- ninb shows where you are looking, then how far to turn
        local text = fields.playerAngle or fields.angle
        if text and fields.angleDelta then
            text = ("%s (-> %s)"):format(text, fields.angleDelta)
        end
        add("Current angle:", text, value_colour)
    end

    -- one label column wide enough for all of them, so the values line up
    local label_width = 0
    for _, entry in ipairs(pairs_out) do
        label_width = math.max(label_width, #entry.label)
    end

    for _, entry in ipairs(pairs_out) do
        table.insert(rows, {
            cell(entry.label, label_colour, 0),
            cell(entry.text, entry.colour, label_width + 1),
        })
    end

    if util.bool(opt(cfg, "show_info", false), false) then
        local width = math.max(16, math.floor(opt(cfg, "wrap_width", 44)))
        for _, message in ipairs(messages or {}) do
            for _, line in ipairs(wrap(message, width)) do
                table.insert(rows, { cell(line, opt(cfg, "header_color", "#8899aaff"), 0) })
            end
        end
    end

    local throw_rows = math.floor(opt(cfg, "throw_rows", 0))
    local throws = type(data) == "table" and data.eyeThrows or nil

    if throw_rows > 0 and type(throws) == "table" and #throws > 0 then
        local header_colour = opt(cfg, "header_color", "#8899aaff")

        if util.bool(opt(cfg, "show_throw_header", true), true) then
            table.insert(rows, {
                cell("x", header_colour, THROW_COLS[1]),
                cell("z", header_colour, THROW_COLS[2]),
                cell("angle", header_colour, THROW_COLS[3]),
                cell("error", header_colour, THROW_COLS[4]),
            })
        end

        -- newest last, which is the order ninb lists them in
        local first = math.max(1, #throws - throw_rows + 1)
        for i = first, #throws do
            local t = throws[i]
            if type(t) == "table" then
                table.insert(rows, {
                    cell(api.fixed(t.xInOverworld, 2), value_colour, THROW_COLS[1]),
                    cell(api.fixed(t.zInOverworld, 2), value_colour, THROW_COLS[2]),
                    cell(api.fixed(t.angle, 2), value_colour, THROW_COLS[3]),
                    cell(api.fixed(t.error, 4), header_colour, THROW_COLS[4]),
                })
            end
        end
    end

    return rows
end

--[[
    one templated line per prediction, for people who want the numbers and
    nothing else.
]]
function Overlay:_compact_rows(data)
    local cfg = self.cfg
    local rows = {}

    local predictions = type(data) == "table" and data.predictions or nil
    if type(predictions) ~= "table" then return rows end

    local count = math.max(1, math.floor(opt(cfg, "shown_predictions", 1)))
    local template = opt(cfg, "template", "{x}, {z}  {certainty}")
    local coords = opt(cfg, "coords", "block")

    for i = 1, math.min(count, #predictions) do
        local fields = api.prediction_fields(predictions[i], data, coords)
        if fields then
            fields.n = tostring(i)
            table.insert(rows, {
                cell(api.render(template, fields), self:_certainty_colour(fields), 0),
            })
        end
    end

    return rows
end

--[[
    build the rows to draw. empty means nothing to show, which is not the same
    as an error.
]]
function Overlay:rows(data, messages)
    local cfg = self.cfg

    local predictions = type(data) == "table" and data.predictions or nil
    local has_prediction = type(predictions) == "table" and #predictions > 0

    if has_prediction then
        if opt(cfg, "layout", "ninbot") == "compact" then
            return self:_compact_rows(data)
        end

        local fields = api.prediction_fields(predictions[1], data,
            opt(cfg, "coords", "block"))
        if fields then
            return self:_ninbot_rows(fields, data, messages)
        end
    end

    local idle = opt(cfg, "idle_text", "")
    if idle ~= "" then
        return { { cell(idle, opt(cfg, "header_color", "#8899aaff"), 0) } }
    end

    return {}
end

--[[
    a filled rectangle. needs the toolwall patch; without it there is no fill
    primitive at all and the panel is simply skipped.
]]
function Overlay:_rect(rect, colour, depth)
    if type(waywall.rect) ~= "function" then
        if not self.warned then
            util.warn("this waywall has no rect(); overlay background skipped, see patches/")
            self.warned = true
        end
        return
    end

    local ok, obj = pcall(waywall.rect, {
        dst = rect,
        color = colour,
        depth = depth,
    })
    if ok and obj then
        table.insert(self.objects, obj)
    end
end

function Overlay:_text(line, x, y, colour, size, depth)
    if line == nil or line == "" then return end

    local cfg = self.cfg
    local ok, obj = pcall(waywall.text, line, {
        x = x,
        y = y,
        color = colour,
        size = size,
        depth = depth,
        -- ignored by an unpatched waywall, which is the graceful outcome
        outline = math.floor(opt(cfg, "outline", 0)),
        outline_color = opt(cfg, "outline_color", "#000000ff"),
    })
    if ok and obj then
        table.insert(self.objects, obj)
    end
end

--[[
    redraw everything. text objects cannot be mutated, so a change of any kind
    means closing and recreating, which is cheap enough at poll rates.
]]
function Overlay:draw(data, now, messages)
    local cfg = self.cfg
    self:clear()

    local rows = self:rows(data, messages)
    if #rows == 0 then return end

    local joined = ""
    for _, row in ipairs(rows) do
        for _, c in ipairs(row) do joined = joined .. c.text .. "\t" end
        joined = joined .. "\n"
    end

    -- stale handling: drop the readout when nothing has changed for a while
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
    local pad = math.floor(opt(cfg, "padding", 6))

    -- rows never overlap: the pitch is the glyph height plus a chosen gap
    local pitch = CHAR_H * size + math.floor(opt(cfg, "line_gap", 2))

    if util.bool(opt(cfg, "background", false), false) then
        -- fixed width face, so the widest row is a character count
        local widest = 0
        for _, row in ipairs(rows) do
            for _, c in ipairs(row) do
                widest = math.max(widest, c.col + #c.text)
            end
        end

        local w = widest * CHAR_W * size + pad * 2
        local h = (#rows - 1) * pitch + CHAR_H * size + pad * 2

        local border = math.floor(opt(cfg, "border_width", 0))
        if border > 0 then
            self:_rect({
                x = x - pad - border,
                y = y - pad - border,
                w = w + border * 2,
                h = h + border * 2,
            }, opt(cfg, "border_color", "#8899aaff"), 7)
        end

        self:_rect({ x = x - pad, y = y - pad, w = w, h = h },
            opt(cfg, "background_color", "#000000b0"), 8)
    end

    for index, row in ipairs(rows) do
        for _, c in ipairs(row) do
            self:_text(c.text, x + c.col * CHAR_W * size, y + (index - 1) * pitch,
                c.colour, size, 9)
        end
    end
end

return M
