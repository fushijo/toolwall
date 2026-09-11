--[[
    toolwall.ninb_api, reading Ninjabrain Bot's HTTP API from inside waywall.

    WHY NOT JUST USE NINB'S WINDOW

    A floating window is subject to show_floating, which is global, so ninb's
    readout appears and vanishes along with the editor. NBTrackr has the same
    problem for the same reason. Text drawn into the scene is not a window at
    all, so it sits over the game on its own terms, and that is what this feeds.

    HOW IT READS

    waywall's Lua has no sockets and exec() returns no handle, so nothing here
    can wait for an answer. Everything therefore goes through a file that some
    other process writes:

      stream  a shell script holds ninb's event stream open and keeps the
              newest event in that file. ninb pushes on every change, so the
              readout sees a change as it happens.

      fetch   one curl per call, fire and forget. Because there is no handle,
              a read afterwards returns the *previous* fetch, which is a poll
              of lag. It covers ninb's boot, and it is the whole fallback when
              streaming cannot work here.

    The two write different files on purpose. Sharing one would make "the
    stream is working" and "a fetch happened once" the same observation, and
    the caller has to be able to tell them apart to know whether it still
    needs to fetch.

    Routes are hyphenated, information-messages rather than informationMessages;
    the camel case spelling is the JSON field and asking for it is a 400. ninb
    1.5.2 serves /api/v1 with stronghold, boat, blind, divine, allAdvancements,
    information-messages, version and ping, and the API is off until it is
    turned on in ninb's own settings.
]]

local waywall = require("waywall")

local json = require("toolwall.json")
local util = require("toolwall.util")

local M = {}

M.DEFAULT_PORT = 52533

-- ninb's route names, which are hyphenated and not the json field names
M.STRONGHOLD = "stronghold"
M.MESSAGES = "information-messages"

--[[
    Where the cache and stream scripts live. Overridable so the test suite can
    keep out of a running session's files.
]]
function M.runtime_dir()
    local dir = os.getenv("XDG_RUNTIME_DIR")
    if not dir or dir == "" then
        return "/tmp"
    end
    return dir
end

--[[
    Where an answer lands. The stream and the one-shot fetch write different
    files on purpose: sharing one would make "the stream is working" and "a
    fetch happened once" indistinguishable, and the fallback has to be able to
    tell them apart.
]]
local function cache_path(name, poll)
    return M.runtime_dir() .. "/toolwall-ninb-" .. name ..
        (poll and "-poll.json" or ".json")
end

function M.now()
    local ok, value = pcall(waywall.current_time)
    if ok and value then return value end
    return os.time() * 1000
end

local function url_for(query, port)
    return ("http://localhost:%d/api/v1/%s"):format(port or M.DEFAULT_PORT, query)
end

--[[
    kick off a one-shot fetch. returns immediately; the answer lands in the
    cache file whenever curl gets round to it, so a read always sees the
    previous fetch. that is a whole poll of lag, which is why streaming below
    exists.
]]
function M.fetch(query, port)
    -- exec splits on spaces, so every token here has to be space-free
    waywall.exec(("curl -s --max-time 2 %s -o %s"):format(
        url_for(query, port), cache_path(query, true)))
end

--[[
    STREAMING

    ninb serves server-sent events at <query>/events and pushes the whole
    document every time it changes. That is the difference between seeing an
    angle change on the next poll and seeing it as it happens, which matters
    when you are spamming F3+C to line a throw up.

    waywall's lua has no sockets and exec() gives no handle, so the stream is
    held by a shell script: curl streams, the loop keeps only the newest event,
    and it lands in the cache file by rename so a read never catches a half
    written line. The file never grows.

    flock is the single-instance guard. Re-running the script while one is
    alive exits immediately, and the lock is released when the holder dies, so
    "start it again" is both the way to start it and the way to restart it
    after ninb or waywall went away.
]]
local function stream_path(query)
    return M.runtime_dir() .. "/toolwall-ninb-" .. query .. ".sh"
end

function M.stream(query, port)
    local path = stream_path(query)
    local out = cache_path(query)

    local fh = io.open(path, "w")
    if not fh then
        return false
    end

    --[[
        waywall gives a child's stdout to /dev/null but lets its stderr through
        to the console, so anything noisy here lands in the user's log. Nothing
        in this script has a reader, so all of it is silenced.

        The retry loop lives in the script on purpose. ninb takes seconds to
        boot, and reconnecting from here costs one shell rather than an exec
        out of lua every few seconds for the life of the session. The count is
        a bound, not a schedule: waywall kills the process it spawned, but the
        shell that was exec'd into is a grandchild and could outlive it, so
        this has to run out on its own eventually.
    ]]
    fh:write(([[
#!/bin/sh
exec 2>/dev/null
exec flock -n '%s.lock' /bin/sh -c '
    attempt=0
    while [ $attempt -lt 200 ]; do
        curl -s -N "%s" | while IFS= read -r line; do
            case "$line" in
                "data: "*) printf "%%s\n" "${line#data: }" > "%s.part" && mv "%s.part" "%s" ;;
            esac
        done
        attempt=$((attempt + 1))
        sleep 3
    done
'
]]):format(out, url_for(query .. "/events", port), out, out, out))
    fh:close()

    waywall.exec("sh " .. path)
    return true
end

--[[
    throw away whatever a previous session left behind, so a stale readout is
    never mistaken for a live one.
]]
function M.forget(query)
    os.remove(cache_path(query))
    os.remove(cache_path(query, true))
end

--[[
    read whatever the last fetch left behind. nil when there is nothing yet,
    or when ninb handed back something that is not json.
]]
function M.read(query, poll)
    local fh = io.open(cache_path(query, poll), "r")
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
    format a number the way the community writes coordinates: no trailing .0,
    one decimal otherwise.
]]
function M.number(value)
    if type(value) ~= "number" then return tostring(value or "") end
    if value == math.floor(value) then return tostring(math.floor(value)) end
    return tostring(round(value, 1))
end

--[[
    format at a fixed number of decimals, the way ninb prints its own table.
    round() alone is not enough: 0.0002 and -16.49 both have to survive.
]]
function M.fixed(value, places)
    if type(value) ~= "number" then return tostring(value or "") end

    local out = ("%." .. tostring(places) .. "f"):format(value)

    -- a tiny negative rounds to "-0.0", which reads as an error rather than
    -- as "you are already pointing the right way"
    if out:match("^%-0%.?0*$") then
        out = out:sub(2)
    end

    return out
end

--[[
    minecraft's yaw: 0 looks towards +z and the value decreases anticlockwise,
    so the heading from one point to another is -atan2(dx, dz).
]]
function M.angle_to(px, pz, tx, tz)
    return -math.deg(math.atan2(tx - px, tz - pz))
end

--[[
    shortest signed difference between two headings, in (-180, 180].
]]
function M.angle_delta(from, to)
    local d = (to - from) % 360
    if d > 180 then d = d - 360 end
    return d
end

--[[
    flatten one prediction into template fields.

    every coordinate space is filled in, because a run crosses all three: the
    stronghold is found in chunk coordinates, walked to in block coordinates
    and travelled to through the nether at an eighth of the scale.
]]
function M.prediction_fields(best, data, coords)
    if type(best) ~= "table" then return nil end

    local fields = {}
    for key, value in pairs(best) do
        if type(value) == "number" then
            fields[key] = M.number(value)
        elseif type(value) ~= "table" and value ~= util.NULL then
            fields[key] = tostring(value)
        end
    end

    if type(best.certainty) == "number" then
        fields.certaintyValue = best.certainty * 100
        fields.certainty = M.number(round(fields.certaintyValue, 1)) .. "%"
    end

    -- ninb reports the stronghold chunk; the block is that chunk's centre, and
    -- the nether portal for it is an eighth of the way out
    local cx, cz = best.chunkX, best.chunkZ
    if type(cx) == "number" and type(cz) == "number" then
        local bx, bz = cx * 16 + 4, cz * 16 + 4

        fields.blockX, fields.blockZ = M.number(bx), M.number(bz)
        fields.netherX = M.number(math.floor(bx / 8))
        fields.netherZ = M.number(math.floor(bz / 8))

        if coords == "chunk" then
            fields.x, fields.z = M.number(cx), M.number(cz)
        elseif coords == "nether" then
            fields.x, fields.z = fields.netherX, fields.netherZ
        else
            fields.x, fields.z = fields.blockX, fields.blockZ
        end
    end

    if type(best.overworldDistance) == "number" then
        fields.distance = M.number(math.floor(best.overworldDistance))
        fields.netherDistance = M.number(math.floor(best.overworldDistance / 8))
    end

    -- the heading to the stronghold, and how far the player has to turn to
    -- reach it. both need the player, which lives outside the prediction.
    local player = type(data) == "table" and data.playerPosition or nil
    if type(player) == "table" and type(player.xInOverworld) == "number" and
        type(best.chunkX) == "number" then
        local angle = M.angle_to(player.xInOverworld, player.zInOverworld,
            best.chunkX * 16 + 4, best.chunkZ * 16 + 4)

        fields.angle = M.fixed(angle, 2)

        if type(player.horizontalAngle) == "number" then
            fields.playerAngle = M.fixed(player.horizontalAngle, 2)
            fields.angleDelta = M.fixed(
                M.angle_delta(player.horizontalAngle, angle), 1)
        end
    end

    fields.throws = tostring(#((type(data) == "table" and data.eyeThrows) or {}))
    return fields
end

--[[
    the information messages ninb shows under the readout, as plain strings.
]]
function M.messages(data)
    local out = {}
    local list = type(data) == "table" and data.informationMessages or nil

    if type(list) == "table" then
        for _, entry in ipairs(list) do
            if type(entry) == "table" and type(entry.message) == "string" then
                table.insert(out, entry.message)
            elseif type(entry) == "string" then
                table.insert(out, entry)
            end
        end
    end

    return out
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
