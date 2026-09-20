--[[
    toolwall.util, small shared helpers.
]]

local M = {}

--[[
    Sentinel for JSON null. Lua tables cannot store nil, so the decoder
    substitutes this and callers compare against it explicitly.
]]
M.NULL = setmetatable({}, {
    __tostring = function() return "null" end,
    __newindex = function() error("attempt to modify null sentinel") end,
})

--[[
    Where warnings go to be found later.

    waywall logs to stderr and nowhere else, so if you started it from a
    desktop entry, every complaint toolwall makes lands in the void. The first bug report on this project arrived as "i dont know
    if there are logs", which is not a thing anyone should have to guess at.
]]
local LOG_MAX = 256 * 1024

-- A function, not a constant, so the tests can point it somewhere that is not
-- the real state directory. They found that out the hard way.
function M.log_path()
    local state = os.getenv("XDG_STATE_HOME")
    if not state or state == "" then
        local home = os.getenv("HOME")
        if not home or home == "" then return nil end
        state = home .. "/.local/state"
    end
    return state .. "/toolwall.log"
end

local function log_to_file(line)
    local path = M.log_path()
    if not path then return end

    -- Appending forever would be a slow leak, and a reload is cheap enough
    -- that checking the size on every warning costs nothing measurable.
    local mode = "a"
    local existing = io.open(path, "r")
    if existing then
        local size = existing:seek("end")
        existing:close()
        if size and size > LOG_MAX then mode = "w" end
    end

    local fh = io.open(path, mode)
    if not fh then return end
    fh:write(os.date("%Y-%m-%d %H:%M:%S "), line, "\n")
    fh:close()
end

function M.warn(message)
    local line = "toolwall: " .. tostring(message)

    -- waywall replaces print() so output is formatted like its own log lines.
    print(line)

    -- Never let logging be the thing that breaks a config load.
    pcall(log_to_file, line)
end

--[[
    Coerce a value that may be missing or the null sentinel into a boolean.
]]
function M.bool(value, default)
    if value == nil or value == M.NULL then
        return default
    end
    return value and true or false
end

--[[
    Expand a leading ~ and $VARS in a path. waywall does no shell processing,
    and neither Prism nor waywall expand ~, so config paths must be resolved
    here or users will hit silent "file not found" behaviour.
]]
function M.expand(path)
    if path == nil or path == M.NULL or path == "" then
        return path
    end

    local home = os.getenv("HOME") or ""

    path = path:gsub("^~", home)
    path = path:gsub("%$([%w_]+)", function(name)
        return os.getenv(name) or ("$" .. name)
    end)
    path = path:gsub("%${([%w_]+)}", function(name)
        return os.getenv(name) or ("${" .. name .. "}")
    end)

    return path
end

--[[
    Expand a command line rather than a single path.

    waywall.exec() splits on spaces into argv, so a command usually carries
    its arguments, "java -jar ~/.config/waywall/resources/ninb.jar". The ~
    is therefore not at the start of the string, where M.expand looks for it.
]]
function M.expand_command(command)
    if command == nil or command == M.NULL or command == "" then
        return command
    end

    local home = os.getenv("HOME")
    if home and home ~= "" then
        command = command:gsub("~/", home .. "/")
    end

    return M.expand(command)
end

function M.shallow_copy(t)
    local out = {}
    for k, v in pairs(t or {}) do
        out[k] = v
    end
    return out
end

return M
