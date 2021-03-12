hooks = hooks or {}
hooks.hooks = hooks.hooks or {}

local assert = assert
local type = type
local pairs = pairs
local unpack = unpack

function hooks.add(event_name, identifier, func)
    assert(type(event_name) == "string")
    assert(type(func) == "function")

    hooks.hooks[event_name] = hooks.hooks[event_name] or {}
    hooks.hooks[event_name][identifier] = func
end

function hooks.call(event_name, ...)
    local event_hooks = hooks.hooks[event_name]

    local ret

    if event_hooks then
        for k,v in pairs(event_hooks) do
            if type(k) == "string" then
                ret = {v(...)}
            else
                if not k then
                    event_hooks[k] = nil
                else
                    ret = {v(k, ...)}
                end
            end

            if ret[1] then
                return unpack(ret)
            end
        end
    end

    return ret
end

function hooks.get_table()
    return hooks.hooks
end

function hooks.remove(event_name, identifier)
    if hooks.hooks[event_name] then
        hooks.hooks[event_name][identifier] = nil
    end
end
