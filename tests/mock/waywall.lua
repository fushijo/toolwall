--[[
    A stand-in for waywall's built-in module.

    It records every call and enforces the two lifecycle rules that are easy to
    violate and painful to debug inside a real session:

      1. Most API functions may not be called during startup.
      2. Scene objects vanish once closed; using one afterwards is invalid.

    This lets the runtime be exercised without Minecraft, a compositor, or a
    GPU.
]]

local M = {}

M.log = {}
M.startup = true
M.listeners = {}
M.resolution = { width = 0, height = 0 }
M.sensitivity = 0
M.floating = false
M.objects = {}
M.next_id = 0
M.state_value = { screen = "title" }

--[[
    Real waywall fires "load" as soon as its own config finishes parsing,
    which is *before* the Minecraft window has connected and mapped a view.
    set_resolution() (and press_key) require that view; calling them from a
    "load" listener throws "cannot set resolution" until mount_view() runs.
    See waywall/wrap.c: wrap_lua_set_res() -> `if (!wrap->view) return 1;`
]]
M.view_ready = false

local function record(name, ...)
    table.insert(M.log, { name = name, args = { ... } })
end

local function guard(name)
    if M.startup then
        error(name .. " cannot be called during startup", 0)
    end
end

function M.reset()
    M.log = {}
    M.startup = true
    M.listeners = {}
    M.resolution = { width = 0, height = 0 }
    M.sensitivity = 0
    M.floating = false
    M.objects = {}
    M.next_id = 0
    M.state_value = { screen = "title" }
    M.view_ready = false
end

--[[
    Fire a registered event, as waywall would.
]]
function M.fire(event, ...)
    for _, fn in ipairs(M.listeners[event] or {}) do
        fn(...)
    end
end

--[[
    Leave startup mode and fire "load", mirroring waywall's real sequence.
    Note this happens before the Minecraft window has connected: view_ready
    stays false until a test calls mount_view().
]]
function M.finish_startup()
    M.startup = false
    M.fire("load")
end

--[[
    Simulate the Minecraft client's Xwayland window connecting and mapping,
    which happens some time after "load" fires.
]]
function M.mount_view()
    M.view_ready = true
end

function M.live_objects()
    local out = {}
    for _, obj in pairs(M.objects) do
        if not obj.closed then
            out[obj.id] = obj
        end
    end
    return out
end

function M.live_count()
    local n = 0
    for _ in pairs(M.live_objects()) do n = n + 1 end
    return n
end

local function new_object(kind, payload)
    M.next_id = M.next_id + 1

    local obj = {
        id = M.next_id,
        kind = kind,
        payload = payload,
        depth = (payload and payload.depth) or 0,
        closed = false,
    }

    function obj:close()
        if self.closed then
            error("scene object used after close", 0)
        end
        self.closed = true
        record("close", self.kind, self.id)
    end

    function obj:get_depth()
        if self.closed then error("scene object used after close", 0) end
        return self.depth
    end

    function obj:set_depth(depth)
        if self.closed then error("scene object used after close", 0) end
        self.depth = depth
    end

    M.objects[obj.id] = obj
    return obj
end

function M.listen(event, fn)
    record("listen", event)
    M.listeners[event] = M.listeners[event] or {}
    table.insert(M.listeners[event], fn)
    return function()
        for i, registered in ipairs(M.listeners[event]) do
            if registered == fn then
                table.remove(M.listeners[event], i)
                return
            end
        end
    end
end

function M.set_resolution(width, height)
    guard("set_resolution")
    if not M.view_ready then
        error("cannot set resolution", 0)
    end
    record("set_resolution", width, height)
    M.resolution = { width = width, height = height }
end

function M.active_res()
    guard("active_res")
    return M.resolution.width, M.resolution.height
end

function M.set_sensitivity(value)
    guard("set_sensitivity")
    record("set_sensitivity", value)
    M.sensitivity = value
end

function M.mirror(options)
    guard("mirror")
    record("mirror", options)
    return new_object("mirror", options)
end

function M.image(path, options)
    guard("image")
    record("image", path, options)

    -- Behave like waywall: a missing PNG is an error, not a blank object.
    local fh = io.open(path, "r")
    if not fh then
        error("failed to load image: " .. tostring(path), 0)
    end
    fh:close()

    local payload = options or {}
    payload.path = path
    return new_object("image", payload)
end

function M.text(body, options)
    guard("text")
    record("text", body, options)
    local payload = options or {}
    payload.body = body
    return new_object("text", payload)
end

function M.exec(command)
    guard("exec")
    record("exec", command)
end

function M.sleep(ms)
    guard("sleep")
    record("sleep", ms)
end

function M.show_floating(show)
    guard("show_floating")
    record("show_floating", show)
    M.floating = show and true or false
end

function M.floating_shown()
    guard("floating_shown")
    return M.floating
end

function M.press_key(key)
    guard("press_key")
    record("press_key", key)
end

function M.get_key(key)
    guard("get_key")
    return false
end

function M.set_keymap(options)
    guard("set_keymap")
    record("set_keymap", options)
end

function M.set_remaps(remaps)
    guard("set_remaps")
    record("set_remaps", remaps)
end

function M.toggle_fullscreen()
    guard("toggle_fullscreen")
    record("toggle_fullscreen")
end

function M.state()
    guard("state")
    return M.state_value
end

function M.current_time()
    return os.time() * 1000
end

function M.profile()
    return nil
end

return M
