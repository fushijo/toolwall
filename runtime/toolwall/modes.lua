--[[
    toolwall.modes — mode switching.

    A "mode" bundles a resolution, an optional sensitivity override, and the
    set of scene objects that should be live while it is selected. This is the
    unit the GUI edits and the unit keybinds address.
]]

local waywall = require("waywall")
local state = require("toolwall.state")
local util = require("toolwall.util")

local Modes = {}
Modes.__index = Modes

local M = {}

function M.new(doc, scene)
    return setmetatable({
        doc = doc,
        scene = scene,
        current = nil,
        listeners = {},
    }, Modes)
end

function Modes:on_change(fn)
    table.insert(self.listeners, fn)
end

function Modes:_emit()
    for _, fn in ipairs(self.listeners) do
        local ok, err = pcall(fn, self.current)
        if not ok then
            util.warn("mode listener error: " .. tostring(err))
        end
    end
end

--[[
    The scene objects that should be live regardless of mode.
]]
function Modes:_base_overlays()
    local out = {}
    for _, id in ipairs(self.doc.base_overlays or {}) do
        table.insert(out, id)
    end
    return out
end

function Modes:_overlays_for(mode)
    local out = self:_base_overlays()
    for _, id in ipairs(mode.mirrors or {}) do
        table.insert(out, id)
    end
    for _, id in ipairs(mode.images or {}) do
        table.insert(out, id)
    end
    return out
end

function Modes:get(id)
    return self.doc._modes[id]
end

function Modes:current_mode()
    if not self.current then return nil end
    return self:get(self.current)
end

--[[
    Select a mode. If the mode is already current and declares toggle = true,
    this resets to the base state instead — matching helpers.toggle_res.
]]
function Modes:set(id)
    local mode = self:get(id)
    if not mode then
        util.warn(("unknown mode %q"):format(tostring(id)))
        return false
    end

    if self.current == id and util.bool(mode.toggle, true) then
        return self:reset()
    end

    local res = mode.resolution or {}

    --[[
        waywall fires the "load" event as soon as its own config finishes
        parsing — before Minecraft's window has connected and mapped a view.
        set_resolution() throws until that view exists, which is why
        toolwall.lua's on_load applying default_mode/reset() must not be able
        to crash the load listener. Fail soft here instead: the next real
        keypress (which only happens once the game is actually visible) will
        retry from a clean, un-mutated state.
    ]]
    local ok, err = pcall(waywall.set_resolution, res.width or 0, res.height or 0)
    if not ok then
        util.warn(("cannot apply mode %q yet: %s"):format(id, tostring(err)))
        return false
    end

    local sens = mode.sensitivity
    if sens and sens ~= util.NULL then
        waywall.set_sensitivity(sens)
    else
        waywall.set_sensitivity(0)  -- 0 restores the configured default
    end

    self.scene.resolution_hint = mode.resolution
    self.scene:set_active(self:_overlays_for(mode))

    self.current = id
    state.write("mode", id)
    self:_emit()
    return true
end

--[[
    Return to the unmodified state: Minecraft stretched to the waywall window,
    default sensitivity, only base overlays live.
]]
function Modes:reset()
    -- See the comment in Modes:set() about the load-event/view-ready race.
    local ok, err = pcall(waywall.set_resolution, 0, 0)
    if not ok then
        util.warn(("cannot reset resolution yet: %s"):format(tostring(err)))
        return false
    end

    waywall.set_sensitivity(0)
    self.scene.resolution_hint = nil
    self.scene:set_active(self:_base_overlays())

    self.current = nil
    state.write("mode", "")
    self:_emit()
    return true
end

--[[
    Step through an ordered list of mode ids. An empty list cycles every mode
    in document order.
]]
function Modes:cycle(ids)
    if not ids or #ids == 0 then
        ids = {}
        for _, mode in ipairs(self.doc.modes) do
            table.insert(ids, mode.id)
        end
    end
    if #ids == 0 then return false end

    local index = 0
    for i, id in ipairs(ids) do
        if id == self.current then
            index = i
            break
        end
    end

    local next_id = ids[(index % #ids) + 1]

    -- Bypass the toggle-off behaviour: cycling should always advance.
    if next_id == self.current then
        return true
    end
    return self:set(next_id)
end

return M
