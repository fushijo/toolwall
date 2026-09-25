--[[
    toolwall.keybinds, turn declarative keybind entries into waywall actions.

    The actions table must be built synchronously, before the "load" event,
    because waywall reads it from the returned config. The handlers themselves
    only run later, so they can safely close over runtime state that does not
    exist yet.
]]

local waywall = require("waywall")

local commands = require("toolwall.commands")
local util = require("toolwall.util")

local M = {}

--[[
    The one command that survives a suspension, because it is the way back.
    Locking yourself out of your own settings menu: a classic.

    Suspending every key including this one would leave no way to unsuspend
    from inside the game, and the editor is where the switch lives.
]]
local ESCAPE_COMMAND = "gui.toggle"

--[[
    Keys that open chat. Watched, not bound.

    The State Output mod cannot tell chat from a crafting table. Both arrive
    as "inworld,gamescreenopen", so the menu rebinds that make searchcrafting
    work are also on in chat, and D types O at the person you were talking to.

    Nothing in the state separates them, but how you got there does. A
    crafting table is a right-click on a block. Chat is a key you press. So
    watch that key.

    The watcher returns false, which waywall reads as "not handled" and passes
    the key straight through. From config_vm_try_action:

        consumed = (!lua_isboolean(coro, -1) || lua_toboolean(coro, -1));

    and on_key only fires on press, so the release is delivered either way and
    nothing can stick. Chat still opens. We just know it is about to.
]]
local CHAT_KEYS = { "T", "slash" }

--[[
    Register the chat watchers, if there is anything for them to protect.

    A config with no menu rebinds and no custom layout has nothing that chat
    could get wrong, so it gets no extra binds at all.

    A key you have already bound to something is left alone. Yours outranks
    ours, and a bind on T that opens the eye overlay never opens chat anyway.

    Only fires while unpaused in a world: pressing T inside the search box
    types a t, and must not convince us a chat window just opened.
]]
local function watch_chat_keys(doc, rt, actions)
    local input = doc.input or {}

    local menu = rt.remaps and rt.remaps.menu or {}
    if not next(menu) and type(input.custom_layout) ~= "table" then
        return
    end

    local keys = input.chat_keys
    if type(keys) ~= "table" then
        keys = CHAT_KEYS
    end

    for _, key in ipairs(keys) do
        if type(key) == "string" and key ~= "" and not actions[key] then
            actions[key] = function()
                local ok, st = pcall(waywall.state)
                if ok and type(st) == "table"
                    and st.screen == "inworld"
                    and st.inworld == "unpaused"
                then
                    rt.chat = true
                end

                if rt.trace then
                    util.warn(("chat key %s: state=%s/%s -> chat=%s"):format(
                        key,
                        ok and tostring(st.screen) or "unreadable",
                        ok and tostring(st.inworld) or "-",
                        tostring(rt.chat)))
                end

                -- never consumed. this is a watcher, not a keybind.
                return false
            end
        end
    end
end

--[[
    doc: parsed config document
    rt:  the runtime state table (populated on "load")
]]
function M.build(doc, rt)
    local actions = {}

    local suspended = util.bool(doc.suspend_keybinds, false)
    local escape = nil

    for _, bind in ipairs(doc.keybinds or {}) do
        local input = bind.input
        local command = bind.command
        local args = bind.args or {}
        local f3_safe = util.bool(bind.f3_safe, true)
        local ingame_only = util.bool(bind.ingame_only, false)

        if suspended and command ~= ESCAPE_COMMAND then
            -- left unbound, so the key reaches Minecraft untouched
        elseif type(input) ~= "string" or input == "" then
            util.warn("keybind is missing an input")
        elseif type(command) ~= "string" or command == "" then
            util.warn(("keybind %q is missing a command"):format(input))
        elseif actions[input] then
            util.warn(("duplicate keybind for %q, ignoring"):format(input))
        else
            if command == ESCAPE_COMMAND then
                escape = input
            end

            actions[input] = function()
                -- The scene does not exist until the load event has fired.
                if not rt.modes then
                    return false
                end

                --[[
                    Do not fire while F3 is held.

                    waywall matches modifiers exactly, so a bind on "B" already
                    ignores Shift, Ctrl and Alt. F3 is an ordinary key rather
                    than a modifier, so nothing stops F3+B from also triggering
                    a bind on B, and reaching for hitboxes would flip you into
                    thin. Returning false passes the key through to Minecraft,
                    so the F3 combo does what it should.
                ]]
                if f3_safe then
                    local held, pressed = pcall(waywall.get_key, "F3")
                    if held and pressed then
                        return false
                    end
                end

                --[[
                    And gore's ingame_only, which is the same idea aimed at
                    menus: a resize key should do nothing on the title screen.

                    waywall only knows what Minecraft is doing if the State
                    Output mod is installed, so a config that asks for this
                    without the mod gets a bind that never fires. Treating an
                    unreadable state as "not in game" is the safe half of that
                    trade: a key that does nothing beats a key that resizes
                    you at the wrong moment.
                ]]
                if ingame_only then
                    local ok, st = pcall(waywall.state)
                    if not ok or type(st) ~= "table" then
                        return false
                    end
                    if st.screen ~= "inworld" or st.inworld ~= "unpaused" then
                        return false
                    end
                end

                local fn = commands.get(command)
                if not fn then
                    util.warn(("unknown command %q"):format(command))
                    return false
                end

                local ok, result = pcall(fn, rt, args)
                if not ok then
                    util.warn(("command %q failed: %s"):format(command, tostring(result)))
                    return false
                end

                -- Explicit false means "pass the input through to Minecraft".
                if result == false then
                    return false
                end
            end
        end
    end

    watch_chat_keys(doc, rt, actions)

    if suspended then
        if escape then
            util.warn(("keybinds suspended, %s still opens the editor"):format(escape))
        else
            util.warn("keybinds suspended, and nothing here opens the editor. " ..
                "run: toolwall set suspend_keybinds false")
        end
    end

    return actions
end

return M
