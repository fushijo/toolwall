--[[
    toolwall.scene — a registry of scene objects addressed by id.

    waywall scene objects have no visibility flag. The only lifecycle methods
    are close(), get_depth() and set_depth(). To "hide" something we therefore
    close it and recreate it later.

    Using negative depth to hide instead is tempting, but negative depth places
    the object *behind* the Minecraft instance, which is only equivalent to
    hidden while Minecraft covers that region of the window. At thin or tall
    resolutions it does not, so the object would reappear over the background.
]]

local waywall = require("waywall")
local util = require("toolwall.util")

local Scene = {}
Scene.__index = Scene

local M = {}

function M.new(doc)
    return setmetatable({
        doc = doc,
        live = {},    -- id -> scene object
        active = {},  -- id -> true
        pinned = {},  -- id -> true, shown by hand rather than by a mode
    }, Scene)
end

local function rect(t)
    if not t then return nil end
    return { x = t.x or 0, y = t.y or 0, w = t.w or 0, h = t.h or 0 }
end

local function shader_of(spec)
    local s = spec.shader
    if s == nil or s == util.NULL or s == "" then return nil end
    return s
end

function Scene:_create_mirror(spec)
    local opts = {
        src = rect(spec.src),
        dst = rect(spec.dst),
        depth = spec.depth,
        shader = shader_of(spec),
    }

    if spec.color_key and spec.color_key ~= util.NULL then
        opts.color_key = {
            input = spec.color_key.input,
            output = spec.color_key.output,
        }
    end

    return waywall.mirror(opts)
end

function Scene:_create_image(spec)
    return waywall.image(util.expand(spec.path), {
        dst = rect(spec.dst),
        depth = spec.depth,
        shader = shader_of(spec),
    })
end

--[[
    Create the object with the given id if it is not already live.
]]
function Scene:show(id)
    if self.live[id] then
        self.active[id] = true
        return self.live[id]
    end

    local obj

    local mirror_spec = self.doc._mirrors[id]
    if mirror_spec then
        local ok, res = pcall(function() return self:_create_mirror(mirror_spec) end)
        if not ok then
            util.warn(("failed to create mirror %q: %s"):format(id, tostring(res)))
            return nil
        end
        obj = res
    end

    local image_spec = self.doc._images[id]
    if not obj and image_spec then
        -- A missing PNG must not take down the whole config.
        local ok, res = pcall(function() return self:_create_image(image_spec) end)
        if not ok then
            util.warn(("failed to create image %q: %s"):format(id, tostring(res)))
            return nil
        end
        obj = res
    end

    if not obj then
        util.warn(("unknown scene object %q"):format(id))
        return nil
    end

    self.live[id] = obj
    self.active[id] = true
    return obj
end

function Scene:hide(id)
    local obj = self.live[id]
    if obj then
        obj:close()
        self.live[id] = nil
    end
    self.active[id] = nil
end

--[[
    Show or hide one overlay by hand, independently of the current mode.

    A pinned overlay survives mode switches, which is the whole point: it is
    the equivalent of a toggle key for a single mirror or image.
]]
function Scene:toggle(id)
    if not (self.doc._mirrors[id] or self.doc._images[id]) then
        util.warn(("unknown overlay %q"):format(tostring(id)))
        return false
    end

    if self.live[id] then
        self.pinned[id] = nil
        self:hide(id)
        return true
    end

    self.pinned[id] = true
    return self:show(id) ~= nil
end

--[[
    Make exactly the given set of ids live, closing everything else.

    ids: array of string
]]
function Scene:set_active(ids)
    local want = {}
    for _, id in ipairs(ids or {}) do
        want[id] = true
    end

    -- Hand-pinned overlays are not the mode's to close.
    for id in pairs(self.pinned) do
        want[id] = true
    end

    for id in pairs(self.live) do
        if not want[id] then
            self:hide(id)
        end
    end

    for id in pairs(want) do
        self:show(id)
    end
end

function Scene:clear()
    for id in pairs(self.live) do
        self:hide(id)
    end
end

return M
