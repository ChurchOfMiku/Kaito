sandbox.env = sandbox.env or {}

sandbox.env.base_env = {
    _VERSION = _VERSION,
    os = {
        clock = os.clock
    },
    async = sandbox.utils.deepcopy(async),
    math = sandbox.utils.deepcopy(math),
    string = sandbox.utils.deepcopy(string),
    table = sandbox.utils.deepcopy(table),
    -- Tables
    setmetatable = setmetatable,
    getmetatable = getmetatable,
    -- Iters
    pairs = pairs,
    ipairs = ipairs,
    next = next,
    -- other
    error = error,
    tostring = tostring,
    tonumber = tonumber,
    type = type
}

sandbox.env.get_env = function()
    local env = sandbox.env.env or {}

    for k,v in pairs(sandbox.env.base_env) do
        env[k] = v
    end

    env._G = env

    return env
end
