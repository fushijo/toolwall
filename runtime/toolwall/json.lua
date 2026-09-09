--[[
    toolwall.json, a decode-only JSON parser.

    waywall's Lua environment has no package manager and no cjson, and its
    package.path only covers the waywall config directory. Rather than ask
    users to vendor a dependency, toolwall ships its own decoder.

    Decode only: the runtime never writes config. The CLI and GUI own the write
    path, where serde handles serialisation properly.
]]

local util = require("toolwall.util")

local M = {}

M.null = util.NULL

local parse_value

local function err(str, pos, message)
    local line = 1
    for _ in str:sub(1, pos):gmatch("\n") do
        line = line + 1
    end
    error(("%s at line %d (byte %d)"):format(message, line, pos), 0)
end

local function skip_ws(str, pos)
    local _, stop = str:find("^[ \t\r\n]*", pos)
    return stop + 1
end

local escape_map = {
    ['"'] = '"',
    ["\\"] = "\\",
    ["/"] = "/",
    b = "\b",
    f = "\f",
    n = "\n",
    r = "\r",
    t = "\t",
}

--[[
    Encode a Unicode code point as UTF-8. Lua 5.1 has no utf8 library.
]]
local function utf8_encode(cp)
    if cp < 0x80 then
        return string.char(cp)
    elseif cp < 0x800 then
        return string.char(
            0xC0 + math.floor(cp / 0x40),
            0x80 + (cp % 0x40)
        )
    elseif cp < 0x10000 then
        return string.char(
            0xE0 + math.floor(cp / 0x1000),
            0x80 + (math.floor(cp / 0x40) % 0x40),
            0x80 + (cp % 0x40)
        )
    else
        return string.char(
            0xF0 + math.floor(cp / 0x40000),
            0x80 + (math.floor(cp / 0x1000) % 0x40),
            0x80 + (math.floor(cp / 0x40) % 0x40),
            0x80 + (cp % 0x40)
        )
    end
end

local function parse_hex4(str, pos)
    local hex = str:sub(pos, pos + 3)
    if not hex:match("^%x%x%x%x$") then
        err(str, pos, "invalid \\u escape")
    end
    return tonumber(hex, 16)
end

local function parse_string(str, pos)
    -- str:sub(pos, pos) == '"'
    pos = pos + 1
    local buf = {}

    while true do
        local char = str:sub(pos, pos)

        if char == "" then
            err(str, pos, "unterminated string")
        elseif char == '"' then
            return table.concat(buf), pos + 1
        elseif char == "\\" then
            local esc = str:sub(pos + 1, pos + 1)

            if esc == "u" then
                local cp = parse_hex4(str, pos + 2)
                pos = pos + 6

                -- Combine surrogate pairs into a single code point.
                if cp >= 0xD800 and cp <= 0xDBFF and str:sub(pos, pos + 1) == "\\u" then
                    local low = parse_hex4(str, pos + 2)
                    if low >= 0xDC00 and low <= 0xDFFF then
                        cp = 0x10000 + (cp - 0xD800) * 0x400 + (low - 0xDC00)
                        pos = pos + 6
                    end
                end

                buf[#buf + 1] = utf8_encode(cp)
            else
                local mapped = escape_map[esc]
                if not mapped then
                    err(str, pos, "invalid escape \\" .. esc)
                end
                buf[#buf + 1] = mapped
                pos = pos + 2
            end
        else
            -- Consume a run of ordinary characters in one go.
            local start = pos
            local stop = str:find('["\\]', pos)
            if not stop then
                err(str, pos, "unterminated string")
            end
            buf[#buf + 1] = str:sub(start, stop - 1)
            pos = stop
        end
    end
end

local function parse_number(str, pos)
    local start, stop = str:find("^%-?%d+%.?%d*[eE]?[%+%-]?%d*", pos)
    if not start then
        err(str, pos, "invalid number")
    end

    local value = tonumber(str:sub(start, stop))
    if not value then
        err(str, pos, "invalid number")
    end

    return value, stop + 1
end

local literals = {
    ["true"] = true,
    ["false"] = false,
    ["null"] = M.null,
}

local function parse_literal(str, pos)
    for word, value in pairs(literals) do
        if str:sub(pos, pos + #word - 1) == word then
            return value, pos + #word
        end
    end
    err(str, pos, "unexpected token")
end

local function parse_array(str, pos)
    local out = {}
    pos = skip_ws(str, pos + 1)

    if str:sub(pos, pos) == "]" then
        return out, pos + 1
    end

    while true do
        local value
        value, pos = parse_value(str, pos)
        out[#out + 1] = value

        pos = skip_ws(str, pos)
        local char = str:sub(pos, pos)

        if char == "]" then
            return out, pos + 1
        elseif char == "," then
            pos = skip_ws(str, pos + 1)
        else
            err(str, pos, "expected ',' or ']'")
        end
    end
end

local function parse_object(str, pos)
    local out = {}
    pos = skip_ws(str, pos + 1)

    if str:sub(pos, pos) == "}" then
        return out, pos + 1
    end

    while true do
        if str:sub(pos, pos) ~= '"' then
            err(str, pos, "expected object key")
        end

        local key
        key, pos = parse_string(str, pos)

        pos = skip_ws(str, pos)
        if str:sub(pos, pos) ~= ":" then
            err(str, pos, "expected ':'")
        end

        pos = skip_ws(str, pos + 1)

        local value
        value, pos = parse_value(str, pos)
        out[key] = value

        pos = skip_ws(str, pos)
        local char = str:sub(pos, pos)

        if char == "}" then
            return out, pos + 1
        elseif char == "," then
            pos = skip_ws(str, pos + 1)
        else
            err(str, pos, "expected ',' or '}'")
        end
    end
end

parse_value = function(str, pos)
    pos = skip_ws(str, pos)
    local char = str:sub(pos, pos)

    if char == "{" then
        return parse_object(str, pos)
    elseif char == "[" then
        return parse_array(str, pos)
    elseif char == '"' then
        return parse_string(str, pos)
    elseif char == "-" or char:match("%d") then
        return parse_number(str, pos)
    elseif char == "" then
        err(str, pos, "unexpected end of input")
    else
        return parse_literal(str, pos)
    end
end

function M.decode(str)
    if type(str) ~= "string" then
        error("expected string, got " .. type(str), 0)
    end

    local value, pos = parse_value(str, 1)

    pos = skip_ws(str, pos)
    if pos <= #str then
        err(str, pos, "trailing garbage")
    end

    return value
end

return M
