sandbox = sandbox or {tasks = {}}

include("./lib/async.lua")

include("./sandbox/utils.lua")
include("./sandbox/env.lua")

local HOOK_EVERY_INSTRUCTION = 1024

sandbox.exec = function(state, fenv, fn)
    local instructions_run = state:get_instructions_run()
    local max_instructions = HOOK_EVERY_INSTRUCTION * 8

    -- Set the function env
    sandbox.utils.setfenv(fn, fenv)

    -- Create the coroutine thread
    local thread = coroutine.create(fn)

    debug.sethook(
        thread,
        function()
            instructions_run = instructions_run + HOOK_EVERY_INSTRUCTION
            state:set_instructions_run(instructions_run)
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

    fenv.http = fenv.http or {}
    fenv.http.fetch = function(url, data)
        return state:http_fetch(url, data or {})
    end
    sandbox.utils.setfenv(fenv.http.fetch, fenv)

    local sandbox = sandbox
    fenv.print_table = function(tbl)
        state:print(sandbox.utils.table_to_string(tbl))
    end
    sandbox.utils.setfenv(fenv.print_table, fenv)

    return fenv
end

sandbox.async_callback = function(state, future, success, ...)
    local args = {...}
    sandbox.run(state, function()
        if success then
            future:__handle_resolve(true, table.unpack(args))
        else
            future:__handle_reject(true, table.unpack(args))
        end
    end)
end

sandbox.run = function(state, source)
    local fenv = update_env(sandbox.env.get_env(), state)

    local fn, err

    if type(source) == "function" then
        fn = source
    else
        fn, err = load("print(" .. source .. ")", "", "t", fenv)

        if not fn then
            fn = load(source, "", "t", fenv)
        end
    
        if not fn then
            state:error(err)
            return
        end
    end

    local succ, thread, res = sandbox.exec(state, fenv, fn)

    if succ then
        -- Update the env
        sandbox.env.env = fenv

        if thread then
            local task_fn = function()
                if coroutine.status(thread) == "dead" then
                    return true
                end

                local fenv = sandbox.env.env
                state:set_state() -- Get Rust to set the registry sandbox state variable
                local succ, thread, res = sandbox.run_coroutine(thread)

                if not succ then
                    sandbox.run(state, function()
                        state:error(tostring(res))
                    end)
                    return true
                end

                if not thread or coroutine.status(thread) == "dead" then
                    return true
                end
            end

            sandbox.tasks[task_fn] = task_fn
        end
    else
        sandbox.run(state, function()
            state:error(tostring(res))
        end)
    end
end

sandbox.think = function()
    local remove = {}

    for k,v in pairs(sandbox.tasks) do
        if v() then
            table.insert(remove, k)
        end
    end

    for _, k in pairs(remove) do
        sandbox.tasks[k] = nil
    end
end
