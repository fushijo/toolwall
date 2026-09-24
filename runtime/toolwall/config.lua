--[[
    toolwall.config, loading, validating and recovering the config document.
]]

local json = require("toolwall.json")
local util = require("toolwall.util")

local M = {}

M.SCHEMA_VERSION = 1

function M.config_dir()
    local xdg = os.getenv("XDG_CONFIG_HOME")
    if xdg and xdg ~= "" then
        return xdg .. "/waywall"
    end
    return (os.getenv("HOME") or ".") .. "/.config/waywall"
end

function M.default_path()
    return M.config_dir() .. "/toolwall.json"
end

local function good_path(path)
    return (path or M.default_path()) .. ".last-good"
end

--[[
    Minimal structural validation.

    This is deliberately shallow: the JSON Schema in schema/ is the real
    contract and the CLI/GUI validate against it before writing. This function
    only catches the failures that would crash the runtime.
]]
function M.validate(doc)
    if type(doc) ~= "table" then
        return nil, "config root is not an object"
    end

    if doc.version ~= M.SCHEMA_VERSION then
        return nil, ("schema version %s, expected %d")
            :format(tostring(doc.version), M.SCHEMA_VERSION)
    end

    for _, key in ipairs({ "modes", "mirrors", "images", "text", "apps", "keybinds" }) do
        if doc[key] ~= nil and type(doc[key]) ~= "table" then
            return nil, key .. " must be an array"
        end
        doc[key] = doc[key] or {}
    end

    -- Index collections by id for O(1) lookup and duplicate detection.
    for _, key in ipairs({ "modes", "mirrors", "images", "text", "apps" }) do
        local seen = {}
        for _, entry in ipairs(doc[key]) do
            if type(entry.id) ~= "string" or entry.id == "" then
                return nil, key .. " entry is missing an id"
            end
            if seen[entry.id] then
                return nil, ("duplicate id %q in %s"):format(entry.id, key)
            end
            seen[entry.id] = entry
        end
        doc["_" .. key] = seen
    end

    -- Cross-reference check: modes may only name overlays that exist.
    for _, mode in ipairs(doc.modes) do
        for _, id in ipairs(mode.mirrors or {}) do
            if not doc._mirrors[id] then
                return nil, ("mode %q references unknown mirror %q"):format(mode.id, id)
            end
        end
        for _, id in ipairs(mode.images or {}) do
            if not doc._images[id] then
                return nil, ("mode %q references unknown image %q"):format(mode.id, id)
            end
        end
    end

    return doc
end

function M.read_file(path)
    local fh, err = io.open(path, "r")
    if not fh then
        return nil, err or ("cannot open " .. path)
    end
    local data = fh:read("*a")
    fh:close()
    return data
end

function M.load(path)
    path = path or M.default_path()

    local data, err = M.read_file(path)
    if not data then
        return nil, err
    end

    local ok, doc = pcall(json.decode, data)
    if not ok then
        return nil, "invalid JSON: " .. tostring(doc)
    end

    return M.validate(doc)
end

function M.load_last_good(path)
    return M.load(good_path(path))
end

--[[
    Snapshot the current config as last-known-good. Called only after a
    successful load, so the snapshot is by definition parseable.
]]
function M.mark_good(path)
    path = path or M.default_path()

    local data = M.read_file(path)
    if not data then return end

    local fh = io.open(good_path(path), "w")
    if not fh then return end
    fh:write(data)
    fh:close()
end

--[[
    Shaders are referenced by filename in the document and read off disk here,
    matching waywall's expectation of raw GLSL source strings.
]]
function M.load_shaders(doc)
    local out = {}

    for name, sources in pairs(doc.shaders or {}) do
        local entry = {}
        for _, stage in ipairs({ "vertex", "fragment" }) do
            local file = sources[stage]
            if file and file ~= util.NULL and file ~= "" then
                local src = M.read_file(util.expand(file))
                    or M.read_file(M.config_dir() .. "/" .. file)
                if src then
                    entry[stage] = src
                end
            end
        end
        if next(entry) then
            out[name] = entry
        end
    end

    for name, entry in pairs(M.threshold_shaders(doc)) do
        out[name] = entry
    end

    return out
end

--[[
    waywall's colour key threshold is a constant in its own shader:

        const float threshold = 0.01;   // glsl/texcopy.frag

    There is no uniform and no option, so the only way to key less strictly is
    to hand waywall a shader that says a different number. Everything else is
    texcopy.frag as it ships, including the transparent else branch that makes
    keying "keep only this colour" rather than "recolour this".

    Custom shaders fall back to waywall's own vertex shader when only a
    fragment is given (scene.c: `fragment ? fragment : SHADER_FRAG_TEXCOPY`),
    so f_src_rgba, f_dst_rgba and f_src_pos all arrive as usual.

    waywall compiles shaders once at startup, which is fine: saving reloads.
]]
local FRAGMENT = [[
precision highp float;

varying vec2 f_src_pos;
varying vec4 f_src_rgba;
varying vec4 f_dst_rgba;

uniform sampler2D u_texture;

const float threshold = %s;

void main() {
    vec4 color = texture2D(u_texture, f_src_pos);

    if (f_dst_rgba.a == 0.0) {
        gl_FragColor = color;
    } else if (all(lessThan(abs(f_src_rgba.rgb - color.rgb), vec3(threshold)))) {
        gl_FragColor = f_dst_rgba;
    } else {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 0.0);
    }
}
]]

M.DEFAULT_THRESHOLD = 0.01

--[[
    The shader name for a threshold, or nil when waywall's own will do.

    Named after the value so two mirrors asking for the same tolerance share
    one compiled program.
]]
function M.threshold_shader(threshold)
    if type(threshold) ~= "number" then
        return nil
    end

    local clamped = math.max(0, math.min(1, threshold))
    if math.abs(clamped - M.DEFAULT_THRESHOLD) < 0.0005 then
        return nil
    end

    return ("toolwall_key_%d"):format(math.floor(clamped * 1000 + 0.5))
end

local function each_color_key(doc, fn)
    for _, mirror in ipairs(doc.mirrors or {}) do
        local keys = mirror.color_keys
        if type(keys) ~= "table" or keys == util.NULL then
            keys = {}
        end
        for _, key in ipairs(keys) do
            fn(mirror, key)
        end
        if mirror.color_key and mirror.color_key ~= util.NULL then
            fn(mirror, mirror.color_key)
        end
    end
end

--[[
    One generated shader per distinct tolerance actually asked for.
]]
function M.threshold_shaders(doc)
    local out = {}

    each_color_key(doc, function(_, key)
        local name = M.threshold_shader(key.threshold)
        if name and not out[name] then
            out[name] = { fragment = FRAGMENT:format(("%.5f"):format(key.threshold)) }
        end
    end)

    return out
end

return M
