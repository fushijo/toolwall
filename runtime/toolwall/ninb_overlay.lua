--[[
    toolwall.ninb_overlay, Ninjabrain Bot's readout drawn into the scene.

    WHY THIS EXISTS

    ninb's own window is a floating window, and waywall's show_floating() is
    global, so ninb appears and vanishes together with every other floating
    window, this project's editor included. Text drawn into the scene is not a
    window, so it stays where it is put.

    WHAT THE FACE GIVES US

    waywall draws Terminus at a fixed 8x16 per character, scaled by a whole
    number. Fixed width means a column is a character count, which is what lets
    labels line up against values and lets the panel be measured rather than
    guessed. A text object carries one colour, so a row is a list of cells,
    each with its own colour and starting column: that is where the grey
    labels, the coloured certainty and the throw table's columns come from.

    WHAT NEEDS THE PATCH

    Filled areas, which is the panel, the border and the separators, need
    waywall.rect from patches/. Stock waywall has no fill primitive at all, so
    without it those are skipped and the text alone is drawn.
]]

local waywall = require("waywall")

local api = require("toolwall.ninb_api")
local util = require("toolwall.util")

local M = {}

local Overlay = {}
Overlay.__index = Overlay

-- waywall's bundled face, before the size multiplier
local CHAR_W = 8
local CHAR_H = 16

-- the eye throw table, as character columns. the angle column is the widest
-- because a nudged throw carries its increment count, as "120.02+2".
local THROW_COLS = { 1, 10, 19, 30 }

-- drawing order within the readout. anything below zero would go behind
-- Minecraft, so these only have to be distinct and in this order.
local DEPTH_BORDER = 7
local DEPTH_PANEL = 8
local DEPTH_RULE = 9
local DEPTH_TEXT = 10

--[[
    Fallbacks for a config written by hand.

    These mirror NinbOverlay::default() in crates/toolwall-core/src/schema.rs,
    which is the source of truth. The editor writes every field, so a config it
    produced never reaches these; they exist so a partial config still draws
    something sensible. Keeping them in one table is the point: they used to be
    written out at each of forty-odd call sites, and three of them had drifted
    away from the schema without anyone noticing.
]]
local DEFAULTS = {
    layout = "ninbot",

    x = 8,
    y = 40,
    size = 2,
    line_gap = 2,
    padding = 8,

    coords = "block",
    idle_text = "no eye throws yet",

    show_location = true,
    show_certainty = true,
    show_nether = true,
    show_distance = true,
    show_angle = false,
    show_info = false,
    wrap_width = 44,

    throw_rows = 0,
    show_throw_header = true,
    show_correction = true,

    shown_predictions = 1,
    template = "{x}, {z}  {certainty}",

    color = "#ffffffff",
    label_color = "#aaaaaaff",
    header_color = "#7f8ea3ff",
    certainty_high_color = "#55ff55ff",
    certainty_mid_color = "#ffaa00ff",
    certainty_low_color = "#ff5555ff",
    certainty_high_above = 80,
    certainty_mid_above = 50,

    outline = 0,
    outline_color = "#000000ff",

    background = true,
    background_color = "#12161ce6",
    border_width = 0,
    border_color = "#7f8ea3ff",

    separators = false,
    separator_color = "#7f8ea380",
    separator_width = 1,

    hide_after_ms = 0,
}

-- pixel counts and row counts, which have to be whole numbers. the certainty
-- thresholds are deliberately not here: 82.5% is a reasonable cut.
local WHOLE = {
    x = true, y = true, size = true, line_gap = true, padding = true,
    wrap_width = true, throw_rows = true, shown_predictions = true,
    outline = true, border_width = true, separator_width = true,
    hide_after_ms = true,
}

-- below these the layout stops making sense: a zero size draws nothing, a zero
-- wrap width loops forever.
local MINIMUM = {
    size = 1,
    separator_width = 1,
    wrap_width = 16,
    shown_predictions = 1,
}

--[[
    Fill in what the config left out, once, so that drawing can read plain
    fields. Types follow the fallback: a boolean default coerces through
    util.bool, a number through tonumber, anything else to a string.
]]
local function resolve(cfg)
    cfg = type(cfg) == "table" and cfg or {}

    local out = {}
    for key, fallback in pairs(DEFAULTS) do
        local value = cfg[key]

        if value == nil or value == util.NULL then
            value = fallback
        elseif type(fallback) == "boolean" then
            value = util.bool(value, fallback)
        elseif type(fallback) == "number" then
            value = tonumber(value) or fallback
        else
            value = tostring(value)
        end

        if WHOLE[key] then
            value = math.floor(value)
        end
        if MINIMUM[key] and value < MINIMUM[key] then
            value = MINIMUM[key]
        end

        out[key] = value
    end

    return out
end

function M.new(cfg)
    return setmetatable({
        cfg = resolve(cfg),
        objects = {},

        -- the last readout seen, and when it last differed, for hide_after_ms
        latest = nil,
        changed_at = 0,

        -- the readout currently on screen, so an unchanged one costs nothing
        drawn = nil,

        warned = false,
    }, Overlay)
end

function Overlay:clear()
    for _, obj in ipairs(self.objects) do
        pcall(function() obj:close() end)
    end

    self.objects = {}
    self.drawn = nil
end

local function cell(text, colour, col)
    return { text = tostring(text or ""), colour = colour, col = col or 0 }
end

-- a rule between sections. it is a row so that it takes part in layout.
local SEPARATOR = { separator = true }

function Overlay:_separator(rows)
    if self.cfg.separators and #rows > 0 then
        table.insert(rows, SEPARATOR)
    end
end

-- colour a certainty by how good it is, the way every other tool does
function Overlay:_certainty_colour(fields)
    local cfg = self.cfg

    local pct = tonumber(fields.certaintyValue)
        or tonumber(tostring(fields.certainty or ""):match("[%d%.]+"))

    if not pct then
        return cfg.color
    elseif pct >= cfg.certainty_high_above then
        return cfg.certainty_high_color
    elseif pct >= cfg.certainty_mid_above then
        return cfg.certainty_mid_color
    end

    return cfg.certainty_low_color
end

-- break a long message onto several lines at word boundaries
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

    if line ~= "" then
        table.insert(lines, line)
    end

    return lines
end

--[[
    One throw's angle, with its correction shown the way ninb shows it.

    Nudging a throw with ninb's hotkeys does not rewrite the angle you
    measured, it counts the increments you moved it by. "120.02+2" is the
    measurement and two nudges up, which is what you need to decide whether to
    nudge again.
]]
function Overlay:_angle_cell(throw)
    local base = throw.angleWithoutCorrection

    if type(base) ~= "number" or not self.cfg.show_correction then
        return api.fixed(throw.angle, 2)
    end

    local increments = tonumber(throw.correctionIncrements) or 0
    if increments == 0 then
        return api.fixed(base, 2)
    end

    return ("%s%+d"):format(api.fixed(base, 2), increments)
end

--[[
    ninb's window rebuilt: a label column beside a value column, then its
    hints, then the eye throws.
]]
function Overlay:_ninbot_rows(fields, data, messages)
    local cfg = self.cfg
    local rows = {}

    -- gathered first, so that one label column can be sized for all of them
    local labelled = {}
    local function add(label, text, colour)
        if text and text ~= "" then
            table.insert(labelled, { label = label, text = text, colour = colour })
        end
    end

    local function place(x, z, distance)
        local where = ("(%s, %s)"):format(x or "?", z or "?")
        if distance and cfg.show_distance then
            where = where .. (", %s blocks away"):format(distance)
        end
        return where
    end

    if cfg.show_location then
        add(cfg.coords == "chunk" and "Chunk:" or "Location:",
            place(fields.x, fields.z, fields.distance), cfg.color)
    end

    if cfg.show_certainty then
        add("Certainty:", fields.certainty, self:_certainty_colour(fields))
    end

    -- the row the overworld-only tools leave out. a run reaches the stronghold
    -- through the nether far more often than on foot.
    if cfg.show_nether and fields.netherX then
        add("Nether coords:",
            place(fields.netherX, fields.netherZ, fields.netherDistance), cfg.color)
    end

    if cfg.show_angle then
        -- where you are looking, then how far to turn
        local text = fields.playerAngle or fields.angle
        if text and fields.angleDelta then
            text = ("%s (-> %s)"):format(text, fields.angleDelta)
        end
        add("Current angle:", text, cfg.color)
    end

    local label_width = 0
    for _, entry in ipairs(labelled) do
        label_width = math.max(label_width, #entry.label)
    end

    for _, entry in ipairs(labelled) do
        table.insert(rows, {
            cell(entry.label, cfg.label_color, 0),
            cell(entry.text, entry.colour, label_width + 1),
        })
    end

    messages = messages or {}
    if cfg.show_info and #messages > 0 then
        self:_separator(rows)

        for _, message in ipairs(messages) do
            for _, line in ipairs(wrap(message, cfg.wrap_width)) do
                table.insert(rows, { cell(line, cfg.header_color, 0) })
            end
        end
    end

    local throws = type(data) == "table" and data.eyeThrows or nil
    if cfg.throw_rows > 0 and type(throws) == "table" and #throws > 0 then
        self:_separator(rows)

        if cfg.show_throw_header then
            table.insert(rows, {
                cell("x", cfg.header_color, THROW_COLS[1]),
                cell("z", cfg.header_color, THROW_COLS[2]),
                cell("angle", cfg.header_color, THROW_COLS[3]),
                cell("error", cfg.header_color, THROW_COLS[4]),
            })
        end

        -- newest last, which is the order ninb lists them in
        for i = math.max(1, #throws - cfg.throw_rows + 1), #throws do
            local throw = throws[i]
            if type(throw) == "table" then
                table.insert(rows, {
                    cell(api.fixed(throw.xInOverworld, 2), cfg.color, THROW_COLS[1]),
                    cell(api.fixed(throw.zInOverworld, 2), cfg.color, THROW_COLS[2]),
                    cell(self:_angle_cell(throw), cfg.color, THROW_COLS[3]),
                    cell(api.fixed(throw.error, 4), cfg.header_color, THROW_COLS[4]),
                })
            end
        end
    end

    return rows
end

-- one templated line per prediction, for people who want the numbers and
-- nothing else
function Overlay:_compact_rows(data)
    local cfg = self.cfg
    local rows = {}

    local predictions = type(data) == "table" and data.predictions or {}

    for i = 1, math.min(cfg.shown_predictions, #predictions) do
        local fields = api.prediction_fields(predictions[i], data, cfg.coords)
        if fields then
            fields.n = tostring(i)
            table.insert(rows, {
                cell(api.render(cfg.template, fields), self:_certainty_colour(fields), 0),
            })
        end
    end

    return rows
end

--[[
    The rows to draw. Empty means there is nothing to show, which is not the
    same as an error: before the first throw there is no prediction to report.
]]
function Overlay:rows(data, messages)
    local cfg = self.cfg
    local predictions = type(data) == "table" and data.predictions or nil

    if type(predictions) == "table" and #predictions > 0 then
        if cfg.layout == "compact" then
            return self:_compact_rows(data)
        end

        local fields = api.prediction_fields(predictions[1], data, cfg.coords)
        if fields then
            return self:_ninbot_rows(fields, data, messages)
        end
    end

    if cfg.idle_text ~= "" then
        return { { cell(cfg.idle_text, cfg.header_color, 0) } }
    end

    return {}
end

--[[
    A filled rectangle. Needs the toolwall patch; stock waywall has no fill
    primitive at all, so without it the area is simply left empty.
]]
function Overlay:_rect(rect, colour, depth)
    if type(waywall.rect) ~= "function" then
        if not self.warned then
            util.warn("this waywall has no rect(); overlay background skipped, see patches/")
            self.warned = true
        end
        return
    end

    local ok, obj = pcall(waywall.rect, { dst = rect, color = colour, depth = depth })
    if ok and obj then
        table.insert(self.objects, obj)
    end
end

function Overlay:_text(line, x, y, colour)
    if line == nil or line == "" then
        return
    end

    local ok, obj = pcall(waywall.text, line, {
        x = x,
        y = y,
        color = colour,
        size = self.cfg.size,
        depth = DEPTH_TEXT,

        -- ignored by an unpatched waywall, which is the graceful outcome
        outline = self.cfg.outline,
        outline_color = self.cfg.outline_color,
    })
    if ok and obj then
        table.insert(self.objects, obj)
    end
end

--[[
    Where every row sits, and how big the whole thing ends up.

    Separators take part in this so that the panel grows to hold them and the
    rows below one are pushed down by exactly the rule's height.
]]
function Overlay:_layout(rows)
    local cfg = self.cfg

    local pitch = CHAR_H * cfg.size + cfg.line_gap
    local rule_pitch = cfg.separator_width + cfg.line_gap

    local placed, y, widest = {}, 0, 0

    for _, row in ipairs(rows) do
        if row.separator then
            table.insert(placed, { separator = true, y = y })
            y = y + rule_pitch
        else
            table.insert(placed, { cells = row, y = y })
            for _, c in ipairs(row) do
                widest = math.max(widest, c.col + #c.text)
            end
            y = y + pitch
        end
    end

    return {
        rows = placed,
        width = widest * CHAR_W * cfg.size,
        -- every row advanced by its own trailing gap, and the last one has
        -- nothing underneath it to be separated from
        height = math.max(0, y - cfg.line_gap),
    }
end

-- what is on screen, as a string, so that an identical readout can be skipped
local function signature(rows)
    local parts = {}

    for _, row in ipairs(rows) do
        if row.separator then
            table.insert(parts, "-")
        else
            for _, c in ipairs(row) do
                table.insert(parts, c.text .. "\t" .. c.colour)
            end
        end
        table.insert(parts, "\n")
    end

    return table.concat(parts, "\t")
end

--[[
    Redraw.

    Text objects cannot be mutated, so any change at all means closing and
    recreating every one of them. That is why an unchanged readout is left
    alone, and it is what makes a fifty millisecond tick cheap.
]]
function Overlay:draw(data, now, messages)
    local cfg = self.cfg
    local rows = self:rows(data, messages)

    if #rows == 0 then
        return self:clear()
    end

    local current = signature(rows)

    if current ~= self.latest then
        self.latest = current
        self.changed_at = now or 0
    end

    -- drop the readout once it has sat unchanged for long enough
    if cfg.hide_after_ms > 0 and now and (now - self.changed_at) > cfg.hide_after_ms then
        return self:clear()
    end

    if current == self.drawn then
        return
    end

    self:clear()

    local x, y, pad = cfg.x, cfg.y, cfg.padding
    local layout = self:_layout(rows)

    -- a rule spans the panel when there is one, and the text otherwise
    local rule_x = cfg.background and x - pad or x
    local rule_w = cfg.background and layout.width + pad * 2 or layout.width

    if cfg.background then
        local w, h = layout.width + pad * 2, layout.height + pad * 2

        if cfg.border_width > 0 then
            local edge = cfg.border_width
            self:_rect({ x = x - pad - edge, y = y - pad - edge,
                         w = w + edge * 2, h = h + edge * 2 },
                cfg.border_color, DEPTH_BORDER)
        end

        self:_rect({ x = x - pad, y = y - pad, w = w, h = h },
            cfg.background_color, DEPTH_PANEL)
    end

    for _, row in ipairs(layout.rows) do
        if row.separator then
            self:_rect({ x = rule_x, y = y + row.y, w = rule_w, h = cfg.separator_width },
                cfg.separator_color, DEPTH_RULE)
        else
            for _, c in ipairs(row.cells) do
                self:_text(c.text, x + c.col * CHAR_W * cfg.size, y + row.y, c.colour)
            end
        end
    end

    self.drawn = current
end

return M
