--[[
    toolwall.launch — starting a program once, across config reloads.

    THE PROBLEM

    Saving from the GUI trips waywall's hot reload, which rebuilds the whole
    Lua VM. Every bit of runtime state goes with it, including "I already
    started the GUI". So the next keypress would launch a second copy, and the
    one after that a third — one more per save.

    Module-level state cannot survive this, because the module itself is
    reloaded. The record has to outlive the VM, so it goes in a file.

    HOW

    waywall.exec() gives us no handle on the child, so we cannot ask it for a
    pid. Instead we exec a tiny generated shell script which records its own
    pid and then `exec`s the real command — `exec` replaces the shell with the
    program while keeping the same pid, so the file ends up holding the pid of
    the thing we actually wanted.

    Liveness is then a question about /proc rather than about our own memory,
    which is the only way to get a truthful answer after a reload. The cmdline
    is checked too, so a recycled pid belonging to some unrelated process does
    not read as "already running".
]]

local util = require("toolwall.util")

local M = {}

local function runtime_dir()
    local dir = os.getenv("XDG_RUNTIME_DIR")
    if dir and dir ~= "" then
        return dir
    end
    return "/tmp"
end

function M.pid_path(id)
    return runtime_dir() .. "/toolwall-" .. id .. ".pid"
end

local function claim_path(id)
    return runtime_dir() .. "/toolwall-" .. id .. ".starting"
end

--[[
    How long a launch is believed to be in progress.

    The pid is recorded by the launcher script, which cannot run until after
    exec returns, so for a moment after launching there is no pid to find. A
    second keypress in that window would see "not running" and start a rival
    copy - which is how you end up with several Ninjabrain Bots.
]]
local CLAIM_MS = 8000

local function claim(id)
    local fh = io.open(claim_path(id), "w")
    if not fh then return end
    fh:write(tostring(M.now()))
    fh:close()
end

local function claimed(id)
    local fh = io.open(claim_path(id), "r")
    if not fh then return false end

    local at = tonumber((fh:read("*a") or ""):match("%-?%d+"))
    fh:close()

    if not at then return false end
    return (M.now() - at) < CLAIM_MS
end

--[[
    Milliseconds from a monotonic clock. Overridable for tests, which have no
    waywall to ask.
]]
function M.now()
    local waywall = package.loaded["waywall"]
    if waywall and waywall.current_time then
        local ok, value = pcall(waywall.current_time)
        if ok and value then return value end
    end
    return os.time() * 1000
end

local function script_path(id)
    return runtime_dir() .. "/toolwall-" .. id .. ".sh"
end

--[[
    The most distinctive token of a command line, used to confirm that a pid
    still belongs to what we started. For "java -jar ~/ninb.jar" that is the
    jar path; for a bare "toolwall-gui" it is the binary itself.
]]
local function signature(command)
    local last
    for token in tostring(command):gmatch("%S+") do
        last = token
    end
    return last
end

local function read_pid(id)
    local fh = io.open(M.pid_path(id), "r")
    if not fh then return nil end

    local body = fh:read("*a") or ""
    fh:close()

    return tonumber(body:match("%d+"))
end

--[[
    The command line of a running process, or nil if there is no such process.

    Overridable so the test suite can stand in for /proc; there is no way to
    fake a process with a chosen command line from inside Lua.
]]
function M.proc_cmdline(pid)
    local fh = io.open("/proc/" .. pid .. "/cmdline", "rb")
    if not fh then return nil end

    local cmdline = (fh:read("*a") or ""):gsub("%z", " ")
    fh:close()

    return cmdline
end

--[[
    Is the program we recorded for this id still alive?
]]
function M.running(id, command)
    local pid = read_pid(id)
    if not pid then return false end

    local cmdline = M.proc_cmdline(pid)
    if not cmdline or cmdline == "" then return false end

    local expect = signature(command)
    if not expect then return true end

    return cmdline:find(expect, 1, true) ~= nil
end

--[[
    Launch `command` unless it is already running, recording its pid.

    Returns true if a new process was started.
]]
function M.once(waywall, id, command)
    if M.running(id, command) or claimed(id) then
        return false
    end

    -- Staked before exec, because the pid cannot exist until afterwards.
    claim(id)

    local path = script_path(id)
    local fh = io.open(path, "w")
    if not fh then
        -- Losing the recording is survivable; not launching at all is not.
        util.warn(("cannot write launch script for %q; launching untracked"):format(id))
        waywall.exec(command)
        return true
    end

    fh:write(([[
#!/bin/sh
echo $$ > '%s'
exec %s
]]):format(M.pid_path(id), command))
    fh:close()

    -- waywall.exec() splits on spaces, so both tokens must be space-free.
    waywall.exec("sh " .. path)
    return true
end

return M
