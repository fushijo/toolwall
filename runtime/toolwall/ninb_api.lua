--[[
    toolwall.ninb_api, read Ninjabrain Bot's HTTP API from inside waywall.

    waywall's lua has no sockets, so this borrows the trick from gore's
    ww_requests: shell out to curl writing into a cache file, then read that
    file back on a later pass. exec does not block and gives us no handle, so a
    read always returns the *previous* fetch. at overlay refresh rates that is
    a frame of lag and nobody notices.

    why bother when NBTrackr already draws this: a floating window is subject
    to show_floating, which is global, so ninb's readout appears and vanishes
    with the editor. text drawn into the scene is not a window at all, so it
    sits over the game on its own terms.

    ninb 1.5.2 serves /api/v1 with stronghold, boat, blind, divine,
    allAdvancements, informationMessages, version and ping. the api is off by
    default and has to be enabled in ninb's settings.
]]

local waywall = require("waywall")

local json = require("toolwall.json")
local util = require("toolwall.util")

local M = {}

M.DEFAULT_PORT = 52533

local function cache_path(name)
    local dir = os.getenv("XDG_RUNTIME_DIR")
    if not dir or dir == "" then
        dir = "/tmp"
    end
    return dir .. "/toolwall-ninb-" .. name .. ".json"
end

--[[
    kick off a fetch. returns immediately; the answer lands in the cache file
    whenever curl gets round to it.
]]
function M.fetch(query, port)
    local url = ("http://localhost:%d/api/v1/%s"):format(port or M.DEFAULT_PORT, query)

    -- exec splits on spaces, so every token here has to be space-free
    waywall.exec(("curl -sS --max-time 2 %s -o %s"):format(url, cache_path(query)))
end

--[[
    read whatever the last fetch left behind. nil when there is nothing yet,
    or when ninb handed back something that is not json.
]]
function M.read(query)
    local fh = io.open(cache_path(query), "r")
    if not fh then return nil end

    local body = fh:read("*a") or ""
    fh:close()

    if body == "" then return nil end

    local ok, data = pcall(json.decode, body)
    if not ok or type(data) ~= "table" then return nil end

    return data
end

local function round(n, places)
    local mult = 10 ^ (places or 0)
    return math.floor(n * mult + 0.5) / mult
end

--[[
    the first prediction, flattened into the fields a text template can use.

    ninb returns predictions sorted best first, so [1] is the one you want on
    screen. an empty list means no throws yet, which is not an error.
]]
function M.stronghold_fields(data)
    if type(data) ~= "table" then return nil end

    local predictions = data.predictions
    if type(predictions) ~= "table" or #predictions == 0 then
        return nil
    end

    local best = predictions[1]
    if type(best) ~= "table" then return nil end

    local fields = {}
    for key, value in pairs(best) do
        if type(value) == "number" then
            fields[key] = tostring(round(value, 1))
        elseif type(value) ~= "table" and value ~= util.NULL then
            fields[key] = tostring(value)
        end
    end

    -- certainty reads as a percentage everywhere else in the community
    if type(best.certainty) == "number" then
        fields.certainty = tostring(round(best.certainty * 100, 1)) .. "%"
    end

    fields.throws = tostring(#(data.eyeThrows or {}))
    return fields
end

--[[
    fill {placeholders} from a field table. an unknown placeholder renders
    empty rather than leaving braces on screen.
]]
function M.render(template, fields)
    return (tostring(template):gsub("{(%w+)}", function(key)
        return fields[key] or ""
    end))
end

return M
