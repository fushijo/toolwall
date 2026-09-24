--[[
    toolwall.scene, a registry of scene objects addressed by id.

    waywall scene objects have no visibility flag. The only lifecycle methods
    are close(), get_depth() and set_depth(). To "hide" something we therefore
    close it and recreate it later.

    Using negative depth to hide instead is tempting, but negative depth places
    the object *behind* the Minecraft instance, which is only equivalent to
    hidden while Minecraft covers that region of the window. At thin or tall
    resolutions it does not, so the object would reappear over the background.
]]

local waywall = require("waywall")
local config = require("toolwall.config")
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

        -- The resolution a mode is switching to, for relative captures made
        -- before waywall has applied the new size.
        resolution_hint = nil,
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

--[[
    Resolve a capture region measured from a corner.

    Minecraft pins its debug HUD to the corners at fixed pixel offsets, the
    pie chart is the same size and the same distance from the bottom-right
    corner at 340x1080 as it is at fullscreen. A region pinned to absolute
    coordinates is therefore correct at exactly one resolution, which is how a
    pie-chart capture ends up showing the debug text instead.

    waywall reports 0x0 for an unset resolution and Lua cannot ask how large
    the window is, so the mode's own configured resolution is the fallback.
]]
function Scene:_anchored_rect(spec)
    local src = spec.src or {}
    local w, h = src.w or 0, src.h or 0

    local res_w, res_h = 0, 0
    local ok, aw, ah = pcall(waywall.active_res)
    if ok then
        res_w, res_h = aw or 0, ah or 0
    end

    if (res_w <= 0 or res_h <= 0) and self.resolution_hint then
        res_w = self.resolution_hint.width or 0
        res_h = self.resolution_hint.height or 0
    end

    if res_w <= 0 or res_h <= 0 then
        return nil, "no known resolution"
    end

    local anchor = spec.src_anchor

    --[[
        "center" puts the region on the crosshair, which is the middle of the
        Minecraft window. This is what an eye measuring view needs: the region
        has to track the centre as the resolution changes, and hand-placing it
        means re-deriving (res - size) / 2 every time.
    ]]
    if anchor == "center" then
        return {
            x = math.floor((res_w - w) / 2),
            y = math.floor((res_h - h) / 2),
            w = w,
            h = h,
        }
    end

    -- Otherwise x and y are distances from the anchored edges to the near edge
    -- of the region, which is how gore's config expresses the same rectangles.
    local x, y = src.x or 0, src.y or 0

    if anchor == "topright" or anchor == "bottomright" then
        x = res_w - x
    end
    if anchor == "bottomleft" or anchor == "bottomright" then
        y = res_h - y
    end

    return { x = x, y = y, w = w, h = h }
end

--[[
    Resolve a destination measured from a corner of the screen.

    The mirror side of this has existed for a while; this is the same idea
    pointed at the window instead of the game. Without it every dst is
    absolute, so a config written at 1920x1080 puts the pie chart off the
    right edge of a 1366 wide laptop and strands it mid-screen on a 3440
    ultrawide. Three people in the waywall discord hit that in one week.

    gui.screen is the only record of how big waywall's window is. waywall
    advertises a fixed 8192x8192 output to its clients, so nothing can ask,
    and active_res() answers for the game rather than the window.
]]
function Scene:_screen_rect(spec)
    local dst = spec.dst or {}
    local w, h = dst.w or 0, dst.h or 0

    local gui = self.doc.gui or {}
    local screen = gui.screen or {}
    local sw = tonumber(screen.w) or 0
    local sh = tonumber(screen.h) or 0

    if sw <= 0 or sh <= 0 then
        return nil, "gui.screen is not set, so there is nothing to anchor to"
    end

    local anchor = spec.dst_anchor

    -- Centred means centred, the same as it does for a capture.
    if anchor == "center" then
        return {
            x = math.floor((sw - w) / 2),
            y = math.floor((sh - h) / 2),
            w = w,
            h = h,
        }
    end

    -- x and y are distances from the anchored edges to the near edge of the
    -- rectangle, matching how src_anchor already reads.
    local x, y = dst.x or 0, dst.y or 0

    if anchor == "topright" or anchor == "bottomright" then
        x = sw - x
    end
    if anchor == "bottomleft" or anchor == "bottomright" then
        y = sh - y
    end

    return { x = x, y = y, w = w, h = h }
end

--[[
    Either rectangle, anchored if it asked to be.
]]
local function anchored(value)
    return value ~= nil and value ~= util.NULL and value ~= ""
end

function Scene:_dst_for(spec)
    if not anchored(spec.dst_anchor) then
        return rect(spec.dst)
    end

    local resolved, err = self:_screen_rect(spec)
    if not resolved then
        error("anchored overlay: " .. err, 0)
    end
    return resolved
end

--[[
    Build the waywall objects for one mirror.

    Returns a list, because a mirror carrying several colour keys is drawn as
    one layer per key: colour keying passes only the matching colour, so
    isolating the pie chart from the world behind it means stacking a layer for
    each of its colours. A crop shader would do it in one pass, but waywall
    compiles shaders at startup only, and every edit here arrives by hot
    reload.
]]

--[[
    A border around the destination rectangle, as four filled bars.

    waywall.rect comes from patches/0001. Stock waywall has no fill primitive
    at all, so without the patch this is skipped rather than failing the
    overlay it belongs to.
]]
local warned_outline = false

local function outline_objects(dst, outline, depth)
    local width = math.floor(tonumber(outline.width) or 0)
    if width <= 0 then
        return {}
    end

    if type(waywall.rect) ~= "function" then
        if not warned_outline then
            warned_outline = true
            util.warn("borders need the rect patch, see patches/apply.sh")
        end
        return {}
    end

    local color = outline.color
    if color == nil or color == util.NULL or color == "" then
        color = "#ffffffff"
    end

    local bars = {
        { x = dst.x - width, y = dst.y - width, w = dst.w + width * 2, h = width },
        { x = dst.x - width, y = dst.y + dst.h,  w = dst.w + width * 2, h = width },
        { x = dst.x - width, y = dst.y,          w = width,            h = dst.h },
        { x = dst.x + dst.w, y = dst.y,          w = width,            h = dst.h },
    }

    local objects = {}
    for _, bar in ipairs(bars) do
        table.insert(objects, waywall.rect({ dst = bar, color = color, depth = depth }))
    end
    return objects
end

function Scene:_create_mirror(spec)
    local src = rect(spec.src)

    if spec.src_anchor and spec.src_anchor ~= util.NULL and spec.src_anchor ~= "" then
        local resolved, err = self:_anchored_rect(spec)
        if not resolved then
            error("anchored capture: " .. err, 0)
        end
        src = resolved
    end

    local base = {
        src = src,
        dst = self:_dst_for(spec),
        depth = spec.depth,
        shader = shader_of(spec),
    }

    local keys = {}
    if spec.color_keys and spec.color_keys ~= util.NULL then
        for _, key in ipairs(spec.color_keys) do
            table.insert(keys, key)
        end
    end
    if #keys == 0 and spec.color_key and spec.color_key ~= util.NULL then
        table.insert(keys, spec.color_key)
    end

    local extra = {}
    if spec.outline and spec.outline ~= util.NULL then
        extra = outline_objects(base.dst, spec.outline, spec.depth)
    end

    if #keys == 0 then
        table.insert(extra, waywall.mirror(base))
        return extra
    end

    local objects = extra
    for _, key in ipairs(keys) do
        local opts = util.shallow_copy(base)
        opts.color_key = { input = key.input, output = key.output }

        -- A tolerance other than waywall's own is a different shader, since
        -- the number is compiled in. An explicit shader wins: someone who
        -- wrote one knows better than we do what it does with the key.
        if not opts.shader then
            opts.shader = config.threshold_shader(key.threshold)
        end

        table.insert(objects, waywall.mirror(opts))
    end
    return objects
end

function Scene:_create_image(spec)
    return { waywall.image(util.expand(spec.path), {
        dst = self:_dst_for(spec),
        depth = spec.depth,
        shader = shader_of(spec),
    }) }
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
    for _, obj in ipairs(self.live[id] or {}) do
        obj:close()
    end
    self.live[id] = nil
    self.active[id] = nil
end

--[[
    Show or hide one overlay by hand, independently of the current mode.

    A pinned overlay survives mode switches, which is the whole point: it is
    the equivalent of a toggle key for a single mirror or image.
]]
--[[
    Toggle several overlays as one, so a single key can bring up a set.

    All of them follow the first thing that is already on screen: if any are
    up, they all go down. Otherwise they all come up. Toggling each
    independently would let them drift out of step until the key stopped
    meaning anything.
]]
function Scene:toggle_all(ids)
    local any = false
    for _, id in ipairs(ids) do
        if self.live[id] then
            any = true
            break
        end
    end

    local ok = true
    for _, id in ipairs(ids) do
        if any then
            if self.live[id] then
                self.pinned[id] = nil
                self:hide(id)
            end
        elseif not self.live[id] then
            self.pinned[id] = true
            ok = (self:show(id) ~= nil) and ok
        end
    end

    return ok
end

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
        --[[
            A relative capture is derived from the resolution, so a live one is
            stale the moment the resolution changes. Closing it here means
            show() rebuilds it against the new size instead of short-circuiting
            on the object that already exists.
        ]]
        local spec = self.doc._mirrors[id]
        local relative = spec and spec.src_anchor and spec.src_anchor ~= util.NULL

        if not want[id] or relative then
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
