sandbox = sandbox or {tasks = {}}

include("./sandbox/utils.lua")
include("./sandbox/env.lua")

local HOOK_EVERY_INSTRUCTION = 1024

sandbox.exec = function(state, fenv, fn)
    local instructions_run = 0
    local max_instructions = HOOK_EVERY_INSTRUCTION * 4

    -- Set the function env
    sandbox.utils.setfenv(fn, fenv)

    -- Create the coroutine thread
    local thread = coroutine.create(fn)

    debug.sethook(
        thread,
        function()
            instructions_run = instructions_run + HOOK_EVERY_INSTRUCTION
            if instructions_run >= max_instructions then
                state:terminate("exec")
                error("Execution quota exceeded")
            end
        end,
        "",
        HOOK_EVERY_INSTRUCTION
    )

    return sandbox.run_coroutine(thread)
end

sandbox.run_coroutine = function(thread)
    -- Execute the first coroutine resume
    local ret = {pcall(coroutine.resume, thread)}

    local succ, err, res

    -- Check if the coroutine completed, otherwise add it to the pool
    if coroutine.status(thread) == "dead" then
        succ, err = ret[1] and ret[2], ret[1] and ret[3] or ret[2]

        if succ then
            res = {table.unpack(ret, 3, #ret)}

            return true, nil, res
        else
            return false, nil, err
        end
    else
        return true, thread, nil
    end
end

local function update_env(fenv, state)
    fenv.print = function(...)
        local out = ""
        local tbl = {...}

        for k, v in pairs(tbl) do
            out = out .. tostring(v)

            if next(tbl, k) ~= nil then
                out = out .. ", "
            end
        end

        state:print(out)
    end
    sandbox.utils.setfenv(fenv.print, fenv)
    fenv.PrintTable = function(tbl)
        state:print(sandbox.utils.table_to_string(tbl))
    end

    return fenv
end

sandbox.run = function(state, source)
    local fenv = update_env(sandbox.env.get_env(), state)

    local fn, err = load("print(" .. source .. ")", "", "t", fenv)

    if not fn then
        fn = load(source, "", "t", fenv)
    end

    if not fn then
        state:error(err)
        state:terminate("")
        return
    end

    local succ, thread, res = sandbox.exec(state, fenv, fn)

    if succ then
        -- Update the env
        sandbox.env.env = fenv

        if thread then
            local task_fn = function()
            end

            sandbox.tasks[task_fn] = task_fn
        else
            -- Succ!
            state:terminate("")
        end
    else
        local fn = function()
            state:error(tostring(res))
        end

        sandbox.utils.setfenv(fn, fenv)

        fn()
        state:terminate("")
    end
end

sandbox.think = function()
    for k,v in pairs(sandbox.tasks) do
        if v() then
            sandbox.tasks[k] = nil
        end
    end
end
