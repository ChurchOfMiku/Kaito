sandbox = sandbox or {}

include("./sandbox/env.lua")
include("./sandbox/utils.lua")

local HOOK_EVERY_INSTRUCTION = 1024

local function result_to_string(res)
    if type(res) == "table" then

    else
        return tostring(res)
    end
end

local function results_to_string(res)
    local len = #res

    if len == 0 then
        return ""
    elseif len == 1 then
        return result_to_string(res[1])
    else
        local results = {}
        local max = 0
        local has_newline = false

        for _, v in pairs(res) do
            local t = result_to_string(v)
            has_newline = has_newline or string.find(t, "\n")
            max = math.max(max, #t)
            table.insert(results, t)
        end

        local print_newlines = has_newline or max > 16

        local out = ""

        for k, v in pairs(res) do
            out = out .. v

            if k ~= len then
                out = out .. ","

                if print_newlines then
                    out = out .. "\n"
                end
            end
        end

        return out
    end
end

sandbox.run = function(state, source)
    -- Get fenv
    local fenv = sandbox.env.get_env()
    local instructions_run = 0
    local max_instructions = HOOK_EVERY_INSTRUCTION * 2

    local fn, err = load(source, "", "t", fenv)

    if not fn then
        fn = load("return " .. source, "", "t", fenv)
    end

    -- Set the function env
    sandbox.utils.setfenv(fn, fenv)

    -- Create the coroutine thread
    local thread = coroutine.create(fn)

    debug.sethook(
        thread,
        function()
            instructions_run = instructions_run + HOOK_EVERY_INSTRUCTION
            if instructions_run >= max_instructions then
                error("Quota exceeded, terminated execution")
            end
        end,
        "",
        HOOK_EVERY_INSTRUCTION
    )

    -- Execute the first coroutine resume
    local ret = {pcall(coroutine.resume, thread)}

    local succ, err, res

    -- Check if the coroutine completed, otherwise add it to the pool
    if coroutine.status(thread) == "dead" then
        succ, err = ret[1] and ret[2], ret[1] and ret[3] or ret[2]

        if succ then
            res = {table.unpack(ret, 3, #ret)}

            state:print(results_to_string(res))
        end
    else
        -- TODO: add it to the pool
    end
end
