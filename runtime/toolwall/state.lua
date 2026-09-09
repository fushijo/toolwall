--[[
    toolwall.state, small facts that must outlive a config reload.

    Saving from the editor rebuilds waywall's Lua VM, so anything held in a
    module-level table is gone by the time the new VM runs. That is fine for
    most things and wrong for a few: which mode you are in, for one. Without
    this, editing a rectangle and watching it apply would knock you back to
    the base resolution on every keystroke.

    Kept in the runtime directory rather than the config directory: this is
    session state, not configuration, and it must never end up in a config
    someone shares.
]]

local M = {}

local function dir()
    local runtime = os.getenv("XDG_RUNTIME_DIR")
    if runtime and runtime ~= "" then
        return runtime
    end
    return "/tmp"
end

local function path(key)
    return dir() .. "/toolwall-state-" .. key
end

function M.write(key, value)
    local fh = io.open(path(key), "w")
    if not fh then return end
    fh:write(tostring(value or ""))
    fh:close()
end

function M.read(key)
    local fh = io.open(path(key), "r")
    if not fh then return nil end

    local value = fh:read("*a") or ""
    fh:close()

    if value == "" then return nil end
    return value
end

return M
