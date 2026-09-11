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

function M.warn(message)
    -- waywall replaces print() so output is formatted like its own log lines.
    print("toolwall: " .. tostring(message))
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
