--[[
    toolwall.hud — persistent text readout drawn with waywall scene objects.

    waywall.text() creates an immutable text object. There is no set_text, so
    updating the HUD means closing every text object and recreating it. That is
    cheap (the font is a bundled bitmap font) but it means the HUD should be
    refreshed on events, never polled in a loop.
]]

local waywall = require("waywall")
local util = require("toolwall.util")

local Hud = {}
Hud.__index = Hud

local M = {}

function M.new(doc, modes)
    local self = setmetatable({
        doc = doc,
        modes = modes,
        objects = {},
        banner_obj = nil,
    }, Hud)

    modes:on_change(function() self:refresh() end)

    return self
end

--[[
    Substitute {placeholders} in a template string.
]]
function Hud:_vars()
    local mode = self.modes:current_mode()
    local width, height = 0, 0

    local ok, w, h = pcall(waywall.active_res)
    if ok then
        width, height = w or 0, h or 0
    end

    local state_name = ""
    local ok_state, state = pcall(waywall.state)
    if ok_state and type(state) == "table" then
        state_name = state.screen or ""
        if state.inworld then
            state_name = state_name .. "/" .. state.inworld
        end
    end

    return {
        mode = mode and (mode.label or mode.id) or (self.doc.hud and self.doc.hud.idle_label) or "base",
        width = tostring(width),
        height = tostring(height),
        res = (width == 0 and height == 0) and "auto" or (width .. "x" .. height),
        sens = mode and mode.sensitivity and tostring(mode.sensitivity)
            or tostring((self.doc.input or {}).sensitivity or 1.0),
        state = state_name,
    }
end

function Hud:_render(template, vars)
    return (template:gsub("{(%w+)}", function(key)
        return vars[key] or ""
    end))
end

function Hud:clear()
    for _, obj in ipairs(self.objects) do
        obj:close()
    end
    self.objects = {}
end

function Hud:refresh()
    self:clear()

    local specs = self.doc.text or {}
    if #specs == 0 then return end

    local vars = self:_vars()

    for _, spec in ipairs(specs) do
        local body = self:_render(spec.template or "", vars)

        -- Skip empty lines so a HUD element can conditionally disappear.
        if body ~= "" then
            local ok, obj = pcall(waywall.text, body, {
                x = spec.x or 0,
                y = spec.y or 0,
                color = spec.color,
                size = spec.size,
                depth = spec.depth,
            })

            if ok then
                table.insert(self.objects, obj)
            else
                util.warn(("failed to draw text %q: %s"):format(spec.id, tostring(obj)))
            end
        end
    end
end

--[[
    A one-off message, used to surface degraded startup rather than failing
    silently. Stays until the next banner or an explicit clear.
]]
function Hud:banner(message)
    if self.banner_obj then
        self.banner_obj:close()
        self.banner_obj = nil
    end

    local cfg = (self.doc.hud or {}).banner or {}

    local ok, obj = pcall(waywall.text, message, {
        x = cfg.x or 8,
        y = cfg.y or 8,
        color = cfg.color or "#ff5555",
        size = cfg.size or 2,
        depth = cfg.depth or 100,
    })

    if ok then
        self.banner_obj = obj
    end
end

function Hud:clear_banner()
    if self.banner_obj then
        self.banner_obj:close()
        self.banner_obj = nil
    end
end

return M
