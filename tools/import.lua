--[[
    Turn an existing waywall config into a toolwall.json.

    Usage: luajit tools/import.lua <config-dir> <output.json>

    WHY THIS RUNS THE CONFIG INSTEAD OF READING IT
    ----------------------------------------------
    A waywall config is a Lua program, not a document. Nothing useful is
    sitting in the file as a literal: gore's generic config - the one most
    runners start from - keeps its settings in an `init.lua` table that it
    passes to a `main.lua` function, which then computes mirror rectangles
    from resolutions and builds its keybinds as closures. Reading the text
    would recover almost none of that.

    So we run it, with a `waywall` module that records instead of doing. Every
    mirror, image and resolution the config would have created gets written
    down with its arguments already evaluated, which is exactly the data
    toolwall.json wants.

    Keybinds are the hard part, because their values are opaque functions. We
    call each one - still inside the recording sandbox, so it cannot reach the
    machine - and classify it by what it tried to do. A function that calls
    `set_resolution(340, 1080)` is a thin bind whatever the config happened to
    name it, and this reads a hand-written config as well as it reads gore's.

    WHAT IT WILL NOT DO
    -------------------
    It never guesses. A keybind whose behaviour does not map onto one of
    toolwall's commands is reported and left out, not approximated, and the
    same goes for a mirror whose shape depends on state we are not simulating.
    The summary at the end is the honest part of this script: everything it
    could not carry across is named, so the config can be finished by hand
    rather than quietly arriving wrong.
]]

local CONFIG_DIR = arg[1]
local OUT_PATH = arg[2]

if not CONFIG_DIR or not OUT_PATH then
    io.stderr:write("usage: import.lua <config-dir> <output.json>\n")
    os.exit(2)
end

-- Anything worth reading afterwards: what could not be carried across, and
-- the few things that came across differently to how they went in.
local notes = {}
local function note(fmt, ...)
    table.insert(notes, select("#", ...) > 0 and fmt:format(...) or fmt)
end

--[[
    Tables that must encode as a JSON object even when they are empty.

    An empty Lua table is both, and guessing wrong here is not cosmetic: the
    schema reads remaps as a map, and serde refuses a document that offers it
    a list instead.
]]
local as_object = {}

local function object(t)
    as_object[t] = true
    return t
end

--[[
    ==== THE RECORDING WAYWALL ====
]]

local rec = {
    -- width and height are absent on the always-on ones.
    mirrors = {},    -- { options, width, height }
    images = {},     -- { path, options, width, height }
    texts = {},      -- { text, options }
    resolutions = {},
}

-- Swapped out per probe so one action's calls do not land in another's log.
local calls = {}

-- True only for the first load, which is the one whose scene we keep.
local collecting = true

local function record(name, ...)
    table.insert(calls, { name = name, args = { ... } })
end

-- What the stubs answer with while a probe runs.
local probe = {
    active_res = { 0, 0 },
    f3_held = false,
    state = { screen = "inworld", inworld = "unpaused" },
}

local function seen_resolution(w, h)
    if type(w) ~= "number" or type(h) ~= "number" then return end
    if w <= 0 or h <= 0 then return end

    for _, r in ipairs(rec.resolutions) do
        if r[1] == w and r[2] == h then return end
    end
    table.insert(rec.resolutions, { w, h })
end

--[[
    A stand-in scene object.

    Configs keep handles so they can `:close()` them later, and gore's does
    exactly that for its keybind overlay, so the handle has to answer to the
    same methods or probing its keybind would throw.
]]
local function scene_object()
    local obj = {}
    function obj:close() end
    function obj:set_text() end
    function obj:set_dst() end
    return obj
end

local waywall_stub = {}

function waywall_stub.set_resolution(w, h)
    record("set_resolution", w, h)
    seen_resolution(w, h)
end

function waywall_stub.active_res()
    return probe.active_res[1], probe.active_res[2]
end

function waywall_stub.get_key(key)
    if key == "F3" then
        return probe.f3_held
    end
    return false
end

function waywall_stub.state()
    return probe.state
end

function waywall_stub.exec(cmd)
    record("exec", cmd)
end

function waywall_stub.set_sensitivity(s)
    record("set_sensitivity", s)
end

function waywall_stub.set_remaps(t)
    record("set_remaps", t)
end

function waywall_stub.set_keymap(t)
    record("set_keymap", t)
end

function waywall_stub.press_key(k)
    record("press_key", k)
end

function waywall_stub.toggle_fullscreen()
    record("toggle_fullscreen")
end

function waywall_stub.show_floating(v)
    record("show_floating", v)
end

function waywall_stub.floating_shown()
    return false
end

function waywall_stub.sleep()
    -- Configs sleep around resizes. Nothing here is real, so do not wait.
end

function waywall_stub.current_time()
    return 0
end

function waywall_stub.profile()
    return nil
end

--[[
    The bare constructors build scene objects that are always on.

    res_mirror and res_image are the conditional ones: they only show at a
    given resolution, which is what a toolwall mode is. Calling waywall.mirror
    directly puts it in the scene at load and leaves it there, which is what
    toolwall calls a base overlay.

    These used to be recorded into the probe log and nowhere else, so a config
    with an always-on pie chart imported as zero mirrors without even a note
    saying so. That is what "all my mirrors are gone" turned out to be.
]]
function waywall_stub.text(text, options)
    record("text", text, options)
    if collecting then
        table.insert(rec.texts, { text = text, options = options })
    end
    return scene_object()
end

function waywall_stub.mirror(options)
    record("mirror", options)
    if collecting then
        table.insert(rec.mirrors, { options = options })
    end
    return scene_object()
end

function waywall_stub.image(path, options)
    record("image", path, options)
    if collecting then
        table.insert(rec.images, { path = path, options = options })
    end
    return scene_object()
end

function waywall_stub.rect(...)
    record("rect", ...)
    return scene_object()
end

function waywall_stub.listen(event, fn)
    -- Run "load" listeners: that is where configs that draw a permanent
    -- overlay do it, and we want those objects recorded. Everything else
    -- fires on state we are not simulating.
    if event == "load" and type(fn) == "function" then
        pcall(fn)
    end
end

local helpers_stub = {}

function helpers_stub.res_mirror(options, width, height)
    if collecting then
        table.insert(rec.mirrors, { options = options, width = width, height = height })
        seen_resolution(width, height)
    end
    return function() end
end

function helpers_stub.res_image(path, options, width, height)
    if collecting then
        table.insert(rec.images, { path = path, options = options, width = width, height = height })
        seen_resolution(width, height)
    end
    return function() end
end

function helpers_stub.toggle_res(width, height, sens)
    seen_resolution(width, height)
    return function()
        record("set_resolution", width, height)
        if sens then record("set_sensitivity", sens) end
    end
end

function helpers_stub.toggle_floating()
    record("toggle_floating")
end

function helpers_stub.ingame_only(fn)
    return function(...)
        -- The real one checks the instance's screen. Mirror that, so a probe
        -- run with a menu state can tell the wrapper apart from a bare
        -- function.
        local st = probe.state
        if type(st) ~= "table" or st.screen ~= "inworld" then
            record("blocked_out_of_game")
            return false
        end
        return fn(...)
    end
end

--[[
    ==== SANDBOX ====

    Loading someone's config means running it, and a config is allowed to
    shell out. waywall.exec is already a stub; these are the ways around it.
    None of them are things an import needs, so all of them are closed.
]]
local real_open = io.open

local function sandbox()
    os.execute = function() return 0 end
    os.remove = function() return true end
    os.rename = function() return true end

    io.popen = function()
        return {
            read = function() return nil end,
            lines = function() return function() return nil end end,
            close = function() return true end,
        }
    end

    io.open = function(path, mode)
        if mode and mode:match("[wa+]") then
            return nil, "refused while importing"
        end
        return real_open(path, mode)
    end
end

--[[
    ==== LOAD THE CONFIG ====
]]

package.path = table.concat({
    CONFIG_DIR .. "/?.lua",
    CONFIG_DIR .. "/?/init.lua",
    package.path,
}, ";")

package.loaded["waywall"] = waywall_stub
package.loaded["waywall.helpers"] = helpers_stub

sandbox()

-- Configs narrate ("Paceman Running"). Their output is not ours, and it
-- would land in the middle of the summary.
local real_print = print
print = function() end

--[[
    Load the config, from scratch, every time.

    Probing a keybind runs it, and a config's keybinds share state with each
    other: gore's "chat mode" key sets a `remaps_active` flag that its resize
    keys check before doing anything. Probe that key first and every resize
    afterwards reads as a key that does nothing, so the import silently loses
    thin, wide and tall.

    Reloading per probe is the only honest fix. Probe order is a `pairs`
    walk, so the alternative is an import whose result depends on hash order.

    Modules the config required have to go too - `main.lua` keeps that flag as
    a module-level local, and a cached copy would carry the mutation across.
]]
local BASE_MODULES = {}
for name in pairs(package.loaded) do BASE_MODULES[name] = true end

local function load_config()
    for name in pairs(package.loaded) do
        if not BASE_MODULES[name] then
            package.loaded[name] = nil
        end
    end

    local chunk, load_err = loadfile(CONFIG_DIR .. "/init.lua")
    if not chunk then
        return nil, "could not read init.lua: " .. tostring(load_err)
    end

    local ok, result = pcall(chunk)
    if not ok then
        return nil, "init.lua could not be run: " .. tostring(result)
    end
    if type(result) ~= "table" then
        return nil, "init.lua did not return a config table"
    end

    return result
end

--[[
    Only the first load builds the scene. The reloads are for probing, and
    their mirrors would otherwise arrive again on every one.
]]
local config, err = load_config()
if not config then
    print = real_print
    io.stderr:write(err .. "\n")
    os.exit(1)
end

collecting = false

--[[
    ==== PROBE THE KEYBINDS ====
]]

--[[
    Run one action and report what it tried to do.

    In a coroutine because waywall.sleep only works inside one, and a config
    that sleeps around its resize would otherwise fail to probe. pcall inside,
    so a bind that depends on something we are not simulating is a blank
    result rather than a dead import.
]]
local function run_action(input, opts)
    calls = {}
    probe.f3_held = opts.f3_held or false
    probe.state = opts.state or { screen = "inworld", inworld = "unpaused" }
    probe.active_res = opts.active_res or { 0, 0 }

    -- Fresh config per probe: see load_config above for why.
    local fresh = load_config()
    local fn = fresh and (fresh.actions or {})[input]
    if type(fn) ~= "function" then
        return calls, "keybind went missing on reload"
    end

    local co = coroutine.create(function() return fn() end)
    local alive, err = coroutine.resume(co)
    if not alive then
        return calls, err
    end
    return calls, nil
end

local function first_call(log, name)
    for _, c in ipairs(log) do
        if c.name == name then return c end
    end
    return nil
end

--[[
    Turn one action into a toolwall keybind.

    Classification is by effect, never by the key it is bound to or the name
    the config gave it, so this does not depend on recognising gore's config
    in particular.
]]
local function classify(input)
    local log = run_action(input, {})

    if #log == 0 then
        return nil, "does nothing that toolwall can express"
    end

    -- Resolution binds first: they are the ones worth getting right.
    local res = first_call(log, "set_resolution")
    if res then
        local w, h = res.args[1], res.args[2]
        if type(w) == "number" and type(h) == "number" and w > 0 and h > 0 then
            return { kind = "mode", width = w, height = h }
        end
        return { kind = "command", command = "mode.reset" }
    end

    if first_call(log, "toggle_fullscreen") then
        return { kind = "command", command = "fullscreen.toggle" }
    end

    local exec = first_call(log, "exec")
    if exec then
        local cmd = tostring(exec.args[1] or "")
        if cmd:lower():match("ninjabrain") then
            return { kind = "command", command = "ninb.toggle", jar = cmd:match("([^%s]+%.jar)") }
        end
        --[[
            Kept rather than dropped. This command was already in the config
            and already running, so carrying it across loses nothing, and the
            runtime refuses to run it until gui.allow_exec says otherwise.

            Dropping it meant a paceman key came back as a command line to
            retype. Now it is a checkbox.
        ]]
        return {
            kind = "command",
            command = "exec",
            args = { command = cmd },
            needs_exec = true,
        }
    end

    if first_call(log, "toggle_floating") or first_call(log, "show_floating") then
        return { kind = "command", command = "floating.toggle" }
    end

    if first_call(log, "set_remaps") then
        return nil, "toggles a second rebind set; set it up under Input as the menu rebinds"
    end

    if first_call(log, "text") then
        return nil, "draws a text overlay, which toolwall keeps under Text rather than on a key"
    end

    return nil, "does something toolwall has no command for"
end

--[[
    Does this bind already refuse to fire while F3 is held?

    Run it twice. A config that guards F3 does nothing on the second run, and
    that difference is the flag - there is nothing in the actions table itself
    that says so.
]]
local function detects_f3_guard(input)
    local held = run_action(input, { f3_held = true })
    local free = run_action(input, { f3_held = false })
    return #held < #free
end

--[[
    And the same trick for a bind wrapped in helpers.ingame_only.
]]
local function is_ingame_only(input)
    local out = run_action(input, { state = { screen = "title" } })
    return first_call(out, "blocked_out_of_game") ~= nil
end

--[[
    ==== BUILD THE DOCUMENT ====
]]

--[[
    Name a resolution the way a runner would.

    Ratios, not a lookup: someone's tall is 384x16384 and someone else's is
    350x12000, and both are tall.
]]
local function mode_name(w, h, taken)
    local base
    if h >= w * 8 then
        base = "tall"
    elseif h > w then
        base = "thin"
    elseif w >= h * 3 then
        base = "wide"
    else
        base = ("res_%dx%d"):format(w, h)
    end

    local name, n = base, 2
    while taken[name] do
        name = ("%s_%d"):format(base, n)
        n = n + 1
    end
    taken[name] = true
    return name
end

local function rect_of(r)
    if type(r) ~= "table" then return nil end
    local x, y, w, h = r.x, r.y, r.w, r.h
    if type(w) ~= "number" or type(h) ~= "number" then return nil end
    return {
        x = math.floor(x or 0),
        y = math.floor(y or 0),
        w = math.floor(w),
        h = math.floor(h),
    }
end

local modes, mirrors, images, texts = {}, {}, {}, {}
local base_overlays = {}
local by_resolution = {}
local taken_names = {}

--[[
    Which mode an always-on object belongs to: none of them.

    Returns the mode when the config asked for a resolution, nil when it did
    not, and reports when it asked for one that no mode ended up using.
]]
local function mode_for(o, what)
    if o.width == nil or o.height == nil then
        return nil, false
    end

    local mode = by_resolution[o.width .. "x" .. o.height]
    if not mode then
        note("a %s for %dx%d was skipped: no mode uses that resolution",
            what, o.width, o.height)
        return nil, true
    end

    return mode, false
end

for _, r in ipairs(rec.resolutions) do
    local w, h = r[1], r[2]
    local id = mode_name(w, h, taken_names)

    local mode = {
        id = id,
        label = id:gsub("^%l", string.upper),
        resolution = { width = w, height = h },
        toggle = true,
        mirrors = {},
        images = {},
    }

    table.insert(modes, mode)
    by_resolution[w .. "x" .. h] = mode
end

for _, m in ipairs(rec.mirrors) do
    local src, dst = rect_of(m.options and m.options.src), rect_of(m.options and m.options.dst)
    local mode, reported = mode_for(m, "mirror")

    if not dst then
        if not reported then note("a mirror could not be read and was skipped") end
    elseif not mode and reported then
        -- already named above
    else
        local id
        if mode then
            id = ("%s_mirror_%d"):format(mode.id, #mode.mirrors + 1)
        else
            id = ("base_mirror_%d"):format(#base_overlays + 1)
        end

        local entry = { id = id, src = src or { x = 0, y = 0, w = 0, h = 0 }, dst = dst }

        if m.options.depth then entry.depth = math.floor(m.options.depth) end
        if m.options.shader then entry.shader = m.options.shader end

        local ck = m.options.color_key
        if type(ck) == "table" and ck.input and ck.output then
            entry.color_key = { input = ck.input, output = ck.output }
        end

        table.insert(mirrors, entry)
        if mode then
            table.insert(mode.mirrors, id)
        else
            table.insert(base_overlays, id)
        end
    end
end

for _, im in ipairs(rec.images) do
    local dst = rect_of(im.options and im.options.dst)
    local mode, reported = mode_for(im, "image overlay")

    if not dst or type(im.path) ~= "string" then
        if not reported then note("an image overlay could not be read and was skipped") end
    elseif not mode and reported then
        -- already named above
    else
        local id
        if mode then
            id = ("%s_image_%d"):format(mode.id, #mode.images + 1)
        else
            id = ("base_image_%d"):format(#base_overlays + 1)
        end

        local entry = { id = id, path = im.path, dst = dst }
        if im.options.depth then entry.depth = math.floor(im.options.depth) end

        table.insert(images, entry)
        if mode then
            table.insert(mode.images, id)
        else
            table.insert(base_overlays, id)
        end
    end
end

--[[
    Text drawn at load, which waywall will not let a config create before
    then. toolwall keeps text as templates, so a literal string comes across
    as a template with no placeholders in it, which renders the same.

    Not a base overlay: base_overlays names mirrors and images, the things a
    mode can switch on and off. Every entry in doc.text is always drawn.
]]
for _, t in ipairs(rec.texts) do
    local opts = t.options
    if type(t.text) ~= "string" or type(opts) ~= "table"
        or type(opts.x) ~= "number" or type(opts.y) ~= "number" then
        note("a text overlay could not be read and was skipped")
    else
        local id = ("text_%d"):format(#texts + 1)
        local entry = {
            id = id,
            template = t.text,
            x = math.floor(opts.x),
            y = math.floor(opts.y),
        }
        if type(opts.color) == "string" then entry.color = opts.color end
        if type(opts.size) == "number" then entry.size = math.floor(opts.size) end
        if type(opts.depth) == "number" then entry.depth = math.floor(opts.depth) end

        table.insert(texts, entry)
    end
end

if #texts > 0 then
    note("%d text overlay(s) came across; they live under Text in the editor "
        .. "now, not in init.lua", #texts)
end

local keybinds = {}
local ninb_jar = nil

for input, fn in pairs(config.actions or {}) do
    if type(fn) ~= "function" then
        note("keybind %q is not a function and was skipped", tostring(input))
    else
        local what, why = classify(input)

        if not what then
            note("keybind %q was left out: %s", tostring(input), why)
        else
            local bind = { input = input, f3_safe = detects_f3_guard(input) }

            if what.kind == "mode" then
                local mode = by_resolution[what.width .. "x" .. what.height]
                if mode then
                    bind.command = "mode.set"
                    bind.args = { mode = mode.id }
                    bind.label = mode.label
                else
                    bind = nil
                    note("keybind %q sets %dx%d, which no mode uses",
                        tostring(input), what.width, what.height)
                end
            else
                bind.command = what.command
                bind.args = what.args
                if what.jar then ninb_jar = what.jar end

                if what.needs_exec then
                    note("keybind %q runs a command, which stays switched off until "
                        .. "you turn on gui.allow_exec in the editor", tostring(input))
                end
            end

            if bind then
                bind.ingame_only = is_ingame_only(input)
                table.insert(keybinds, bind)
            end
        end
    end
end

--[[
    Make sure something opens the editor.

    No waywall config binds a toolwall command, so every import lands with no
    key that opens the GUI - and the GUI is where everything below gets put
    back by hand. Without this you have to go find the binary yourself, which
    is not a thing anyone should have to guess at.
]]
local EDITOR_KEYS = {
    "Ctrl-I", "Ctrl-semicolon", "Ctrl-apostrophe", "Ctrl-slash", "Ctrl-period",
}

-- waywall's modifier names, folded onto one spelling each.
local MODIFIER_WORDS = {
    shift = "shift", caps = "caps", lock = "caps", capslock = "caps",
    control = "ctrl", ctrl = "ctrl", alt = "alt", mod1 = "alt",
    num = "num", numlock = "num", mod2 = "num", mod3 = "mod3",
    super = "super", win = "super", mod4 = "super", mod5 = "mod5",
}

-- An input string split into its key and the modifiers it asks for.
local function split_bind(input)
    local key, mods, wild = nil, {}, false

    for elem in tostring(input):gmatch("[^-]+") do
        local lower = elem:lower()
        if elem == "*" then
            wild = true
        elseif MODIFIER_WORDS[lower] then
            mods[MODIFIER_WORDS[lower]] = true
        else
            key = lower
        end
    end

    return key, mods, wild
end

--[[
    Would this existing bind still fire if we took the candidate key?

    waywall matches modifiers exactly, so Shift-I and Ctrl-I are different
    keys and neither shadows the other. A bind carrying "*" is the exception:
    its modifiers only have to be a subset of what is held, so *-I fires on
    Ctrl-I too and that key is not ours to take.
]]
local function shadowed_by(existing, candidate)
    local their_key, their_mods, wild = split_bind(existing)
    local our_key, our_mods = split_bind(candidate)

    if their_key ~= our_key then return false end

    if wild then
        for mod in pairs(their_mods) do
            if not our_mods[mod] then return false end
        end
        return true
    end

    for mod in pairs(their_mods) do
        if not our_mods[mod] then return false end
    end
    for mod in pairs(our_mods) do
        if not their_mods[mod] then return false end
    end
    return true
end

local function add_editor_keybind()
    for _, bind in ipairs(keybinds) do
        if bind.command == "gui.toggle" then return end
    end

    for _, candidate in ipairs(EDITOR_KEYS) do
        local free = true
        for _, bind in ipairs(keybinds) do
            if shadowed_by(bind.input, candidate) then
                free = false
                break
            end
        end

        if free then
            table.insert(keybinds, {
                input = candidate,
                command = "gui.toggle",
                f3_safe = true,
                label = "Open settings",
            })
            note("nothing in your config opened the editor, so %s now does", candidate)
            return
        end
    end

    note("every key toolwall would use for the editor was already taken, so no "
        .. "key opens it; bind gui.toggle by hand or run toolwall-gui yourself")
end

add_editor_keybind()

-- Stable order, so re-importing the same config produces the same file.
table.sort(keybinds, function(a, b) return a.input < b.input end)

--[[
    ==== CARRY THE PLAIN SETTINGS ACROSS ====
]]

local input_cfg = config.input or {}
local theme_cfg = config.theme or {}
local window_cfg = config.window or {}
local experimental_cfg = config.experimental or {}

local src_root = os.getenv("TOOLWALL_SRC")
    or (arg[0] or ""):match("^(.*)/tools/import%.lua$")
    or "."
local keycodes = dofile(src_root .. "/runtime/toolwall/keycodes.lua")

--[[
    Rebinds are the one place an import has to correct as well as copy.

    They are matched against waywall's keycode table, so a name that came from
    the keybind vocabulary aborts the whole config load rather than failing on
    its own. Anything unrecognised is dropped here with a note, because the
    alternative is a toolwall.json that cannot start.
]]
local function carry_remaps(t, label)
    local out = object({})
    if type(t) ~= "table" then return out end

    for from, to in pairs(t) do
        if type(from) ~= "string" or type(to) ~= "string" then
            note("%s under %q is not a pair of names and was dropped", label, tostring(from))
        elseif to == "" then
            note("%s %q had no target key and was dropped", label, from)
        elseif keycodes.valid(from) and keycodes.valid(to) then
            out[from] = to
        else
            note("%s %q -> %q is not a name waywall accepts and was dropped",
                label, from, to)
        end
    end

    return out
end

local doc = {
    version = 1,
    meta = object({
        name = "imported",
        description = "Imported from " .. CONFIG_DIR,
    }),
    input = {
        layout = input_cfg.layout or "",
        model = input_cfg.model or "",
        rules = input_cfg.rules or "",
        variant = input_cfg.variant or "",
        options = input_cfg.options or "",
        remaps = carry_remaps(input_cfg.remaps, "rebind"),
        remaps_menu = object({}),
        repeat_rate = input_cfg.repeat_rate or -1,
        repeat_delay = input_cfg.repeat_delay or -1,
        sensitivity = input_cfg.sensitivity or 1.0,
        confine_pointer = input_cfg.confine_pointer or false,
    },
    theme = {
        background = theme_cfg.background or "#000000",
        background_png = theme_cfg.background_png or "",
        cursor_theme = theme_cfg.cursor_theme or "",
        cursor_icon = theme_cfg.cursor_icon or "",
        cursor_size = theme_cfg.cursor_size or 0,
        ninb_opacity = theme_cfg.ninb_opacity or 1.0,
        ninb_hidden = false,
    },
    window = {
        fullscreen_width = window_cfg.fullscreen_width or 0,
        fullscreen_height = window_cfg.fullscreen_height or 0,
    },
    experimental = {
        debug = experimental_cfg.debug or false,
        jit = experimental_cfg.jit or false,
        tearing = experimental_cfg.tearing or false,
    },
    --[[
        The editor has to be launchable, and nothing in a waywall config says
        how.

        Leaving this out is not harmless: the runtime reads toolwall.json
        directly, so an absent gui block means gui.toggle warns "no
        gui.command configured" and the key added above does nothing at all.

        The path is spelled out because waywall exec()s with its own PATH,
        which usually does not have ~/.cargo/bin on it.
    ]]
    gui = {
        command = "~/.cargo/bin/toolwall-gui",
        launch_delay_ms = 400,
        allow_exec = false,
    },
    default_mode = nil,
    base_overlays = base_overlays,
    modes = modes,
    mirrors = mirrors,
    images = images,
    text = texts,
    keybinds = keybinds,
}

if type(theme_cfg.ninb_anchor) == "table" then
    doc.theme.ninb_anchor = {
        position = theme_cfg.ninb_anchor.position,
        x = theme_cfg.ninb_anchor.x,
        y = theme_cfg.ninb_anchor.y,
    }
elseif type(theme_cfg.ninb_anchor) == "string" then
    doc.theme.ninb_anchor = theme_cfg.ninb_anchor
end

if ninb_jar then
    doc.ninb = { jar = ninb_jar }
end

--[[
    ==== WRITE IT ====

    A small encoder rather than a dependency. Key order is fixed per table so
    that importing the same config twice produces the same bytes, which is
    what makes the output reviewable in a diff.
]]

local ORDER = {
    "version", "meta", "input", "theme", "window", "experimental", "ninb",
    "default_mode", "base_overlays", "modes", "mirrors", "images", "text",
    "keybinds", "hud", "gui",
    -- nested
    "id", "label", "name", "description", "path", "template",
    "resolution", "width", "height", "toggle", "sensitivity",
    "src_anchor", "src", "dst", "depth", "shader", "color_key", "color_keys",
    "x", "y", "w", "h", "input_", "command", "args", "f3_safe", "ingame_only",
}

local rank = {}
for i, key in ipairs(ORDER) do rank[key] = i end

local function escape(s)
    return (s:gsub('[%c"\\]', function(c)
        local map = { ['"'] = '\\"', ["\\"] = "\\\\", ["\n"] = "\\n", ["\r"] = "\\r", ["\t"] = "\\t" }
        return map[c] or ("\\u%04x"):format(c:byte())
    end))
end

local function is_array(t)
    if as_object[t] then return false end
    return #t > 0 or next(t) == nil
end

local encode

local function encode_table(t, indent)
    local pad = ("  "):rep(indent + 1)
    local close = ("  "):rep(indent)

    if is_array(t) then
        if #t == 0 then return "[]" end
        local parts = {}
        for _, v in ipairs(t) do
            table.insert(parts, pad .. encode(v, indent + 1))
        end
        return "[\n" .. table.concat(parts, ",\n") .. "\n" .. close .. "]"
    end

    local keys = {}
    for k in pairs(t) do table.insert(keys, k) end

    table.sort(keys, function(a, b)
        local ra, rb = rank[a] or 500, rank[b] or 500
        if ra ~= rb then return ra < rb end
        return tostring(a) < tostring(b)
    end)

    if #keys == 0 then return "{}" end

    local parts = {}
    for _, k in ipairs(keys) do
        table.insert(parts, ('%s"%s": %s'):format(pad, escape(tostring(k)), encode(t[k], indent + 1)))
    end
    return "{\n" .. table.concat(parts, ",\n") .. "\n" .. close .. "}"
end

encode = function(v, indent)
    indent = indent or 0
    local kind = type(v)

    if v == nil then return "null" end
    if kind == "boolean" then return tostring(v) end
    if kind == "string" then return '"' .. escape(v) .. '"' end
    if kind == "number" then
        if v % 1 == 0 then return ("%d"):format(v) end
        return (("%.7g"):format(v))
    end
    if kind == "table" then return encode_table(v, indent) end

    return "null"
end

local out = assert(real_open(OUT_PATH, "w"), "cannot write " .. OUT_PATH)
out:write(encode(doc, 0), "\n")
out:close()

--[[
    ==== REPORT ====
]]

print = real_print

print(("imported %d mode(s), %d mirror(s), %d overlay image(s), %d text, %d keybind(s)")
    :format(#modes, #mirrors, #images, #texts, #keybinds))

for _, mode in ipairs(modes) do
    print(("  %-10s %dx%d  (%d mirror(s), %d image(s))")
        :format(mode.label, mode.resolution.width, mode.resolution.height,
            #mode.mirrors, #mode.images))
end

-- Otherwise the per-mode lines above add up to less than the total and it
-- reads like something went missing.
if #base_overlays > 0 then
    print(("  %-10s %d overlay(s) that are on in every mode"):format("always on", #base_overlays))
end

--[[
    The notes are the useful half of this script, and they scroll past.

    So they also go in a file next to the config. "what did the import drop"
    has to stay answerable a week later, when the thing you notice missing is
    a key you rarely press.
]]
local REPORT_PATH = CONFIG_DIR .. "/toolwall-import-report.txt"

local report = real_open(REPORT_PATH, "w")
if report then
    report:write(("toolwall import of %s\n"):format(CONFIG_DIR))
    report:write(("%d mode(s), %d mirror(s), %d overlay image(s), %d text, %d keybind(s)\n")
        :format(#modes, #mirrors, #images, #texts, #keybinds))
    if #base_overlays > 0 then
        report:write(("%d of those overlays are on in every mode\n"):format(#base_overlays))
    end

    if #notes > 0 then
        report:write("\nworth knowing:\n")
        for _, n in ipairs(notes) do
            report:write("  - " .. n .. "\n")
        end
    else
        report:write("\neverything carried across cleanly.\n")
    end

    report:write("\nYour old config is still on disk. init.lua was backed up to\n")
    report:write("init.lua.pre-toolwall, and uninstall.sh puts it back.\n")
    report:close()
end

if #notes > 0 then
    print("\nworth knowing:")
    for _, n in ipairs(notes) do
        print("  - " .. n)
    end
end

print("\nsaved to " .. REPORT_PATH)
