--[[
    toolwall.launch, starting a program once, across config reloads.

    THE PROBLEM

    Saving from the GUI trips waywall's hot reload, which rebuilds the whole
    Lua VM. Every bit of runtime state goes with it, including "I already
    started the GUI". So the next keypress would launch a second copy, and the
    one after that a third, one more per save.

    Module-level state cannot survive this, because the module itself is
    reloaded. The record has to outlive the VM, so it goes in a file.

    HOW

    waywall.exec() gives us no handle on the child, so we cannot ask it for a
    pid. Instead we exec a tiny generated shell script which records its own
    pid and then `exec`s the real command, `exec` replaces the shell with the
    program while keeping the same pid, so the file ends up holding the pid of
    the thing we actually wanted.

    Liveness is then a question about /proc rather than about our own memory,
    which is the only way to get a truthful answer after a reload. The cmdline
    is checked too, so a recycled pid belonging to some unrelated process does
    not read as "already running".
]]

local util = require("toolwall.util")

local M = {}

--[[
    Where pidfiles, claims and launch scripts live.

    A module field rather than a local so the tests can point it somewhere
    disposable. They could not before, and every run left pidfiles in the real
    XDG_RUNTIME_DIR: a test that launched something once made every later run
    believe it was still running.
]]
function M.runtime_dir()
    local dir = os.getenv("XDG_RUNTIME_DIR")
    if dir and dir ~= "" then
        return dir
    end
    return "/tmp"
end

function M.pid_path(id)
    return M.runtime_dir() .. "/toolwall-" .. id .. ".pid"
end

--[[
    Which waywall session started the thing at pid_path(id).

    Ninjabrain Bot is not our child; waywall.exec() hands it off and we never
    hear from it again. So when waywall itself dies and comes back - a crash,
    a full reset, just closing and relaunching the instance - ninb does not
    die with it. It is left running, still holding an X11 connection (the
    JNativeHook global-hotkey hook, and its own Swing window) open to the X
    server that waywall has just torn down.

    Without this, M.once saw that old, orphaned ninb was still alive, matched
    the command line, and called it "already running" forever. It never
    starts a working one again, and the orphan is not merely idle: every
    hs_err log on this machine (eleven of them, going back weeks) is
    Ninjabrain Bot, SIGSEGV, on the JNativeHook hook thread or its AWT
    sibling - both blocked on an X server that is no longer there. glibc's
    IO-error handling for a dead X connection does not cope with running on a
    background thread, and takes the whole JVM down with it instead of
    exiting cleanly.

    The owner file is what lets `once` tell "still mine, across a config
    reload" from "somebody else's, and it is not coming back".
]]
local function owner_path(id)
    return M.runtime_dir() .. "/toolwall-" .. id .. ".owner"
end

local function read_owner(id)
    local fh = io.open(owner_path(id), "r")
    if not fh then return nil end
    local body = fh:read("*a") or ""
    fh:close()
    return tonumber(body:match("%d+"))
end

local function write_owner(id, pid)
    local fh = io.open(owner_path(id), "w")
    if not fh then return end
    fh:write(tostring(pid))
    fh:close()
end

--[[
    Ask a process to stop. Overridable, so a test can watch this happen
    without actually sending a signal to anything.

    A plain kill, not -9. Whatever old ninb is on the other end, the point is
    to let it close its X11 connections on its own terms - a shutdown hook
    unregistering the hotkey hook, Swing tearing its window down - rather
    than have the socket vanish out from under a blocking read, which is the
    abrupt kind of disconnect that crashes it in the first place.
]]
function M.kill(pid)
    os.execute(("kill %d >/dev/null 2>&1"):format(pid))
end

local function claim_path(id)
    return M.runtime_dir() .. "/toolwall-" .. id .. ".starting"
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
    return M.runtime_dir() .. "/toolwall-" .. id .. ".sh"
end

--[[
    A command line safe to put inside single quotes in the launch script.
]]
local function shell_quoted(command)
    return "'" .. tostring(command):gsub("'", "'\\''") .. "'"
end

--[[
    waywall's own pid, for things that should happen once per waywall rather
    than once per config reload. This Lua runs inside waywall, so /proc/self
    is waywall.
]]
function M.self_pid()
    local fh = io.open("/proc/self/stat", "r")
    if not fh then return nil end

    local line = fh:read("*l") or ""
    fh:close()
    return tonumber(line:match("^(%d+)"))
end

--[[
    True the first time this is asked for a key in this waywall, false after.

    Every save reloads the config and rebuilds the Lua VM, so a "load"
    listener runs again on every edit. For something idempotent that is fine.
    For a toggle it is not: going fullscreen on start would come back out of
    fullscreen on the next save, back in on the one after, which is what it
    did.

    The marker carries the pid, so a new waywall starts fresh. Old ones are
    left behind until the runtime directory is cleared at logout, and they are
    empty files in a tmpfs.
]]
function M.first_time(key)
    local pid = M.self_pid()
    if not pid then
        -- Cannot tell, so do the thing. Better twice than never.
        return true
    end

    local path = M.runtime_dir() .. "/toolwall-" .. key .. "-" .. pid
    local existing = io.open(path, "r")
    if existing then
        existing:close()
        return false
    end

    local fh = io.open(path, "w")
    if fh then
        fh:write("1")
        fh:close()
    end
    return true
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

    --[[
        Whole command line, not a substring of it. "toolwall-gui" is a
        substring of "toolwall-gui --overlay", so a loose match had each of
        those two believing the other was itself.
    ]]
    local want = tostring(command)
    if want == "" then return true end

    -- /proc turns the separators into spaces and leaves one on the end.
    return (cmdline:gsub("%s+$", "")) == want
end

--[[
    The command that starts Ninjabrain Bot, or nil when there is no jar.

    Two callers want this, the autostart on load and the toggle keybind, and
    they have to agree exactly: `once` recognises an already running copy by
    the last token of the command, so any difference between them reads as a
    different program and starts a second one.
]]
function M.ninb_command(ninb)
    local jar = ninb and ninb.jar
    if not jar or jar == util.NULL or jar == "" then
        return nil
    end

    local template = ninb.command
    if not template or template == util.NULL or template == "" then
        template = "java -jar {jar}"
    end

    return util.expand_command((template:gsub("{jar}", util.expand(jar))))
end

--[[
    Launch `command` unless it is already running, recording its pid.

    Returns true if a new process was started.
]]
function M.once(waywall, id, command)
    local self_pid = M.self_pid()
    local owner = read_owner(id)

    if M.running(id, command) then
        --[[
            Ours, or unowned (an older toolwall's pidfile, before this
            existed) - leave it running, exactly as before.

            Somebody else's and that somebody is not this waywall: it is an
            orphan, and it is not going to notice its X server is gone on its
            own. Ask it to stop, then fall through and launch a live one.
        ]]
        if not (self_pid and owner and owner ~= self_pid) then
            return false
        end

        local stale = read_pid(id)
        if stale then
            M.kill(stale)
        end
    elseif claimed(id) then
        return false
    end

    -- Staked before exec, because the pid cannot exist until afterwards.
    claim(id)
    if self_pid then
        write_owner(id, self_pid)
    end

    local path = script_path(id)
    local fh = io.open(path, "w")
    if not fh then
        -- Losing the recording is survivable; not launching at all is not.
        util.warn(("cannot write launch script for %q; launching untracked"):format(id))
        waywall.exec(command)
        return true
    end

    --[[
        the script checks for an existing copy before starting one.

        the pidfile can go missing (cleared by hand, tmpfs wiped) and then a
        keypress sees "not running" and starts a rival. the shell can ask the
        process table directly, which lua cannot, so the guard lives here.

        -x as well as -f, so the pattern has to match the whole command line
        and not appear anywhere in it. the editor is "toolwall-gui" and the
        screen overlay is "toolwall-gui --overlay", so a substring match made
        each one look like the other was already running: opening the overlay
        killed the editor's keybind, and the editor killed the overlay's, and
        neither said anything about it.

        this script's own line is "sh <path>", so it cannot match itself.
    ]]
    fh:write(([[
#!/bin/sh
if pgrep -x -f %s >/dev/null 2>&1; then exit 0; fi
echo $$ > '%s'
exec %s
]]):format(shell_quoted(command), M.pid_path(id), command))
    fh:close()

    -- waywall.exec() splits on spaces, so both tokens must be space-free.
    waywall.exec("sh " .. path)
    return true
end

return M
