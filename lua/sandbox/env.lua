sandbox.env = sandbox.env or {}

sandbox.env.base_env = {
    _VERSION = _VERSION,
    os = {
        clock = os.clock,
        time = os.time
    },
    async = async,
    bot = bot,
    image = image,
    math = math,
    string = string,
    table = table,
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

function sandbox.env.get_env()
    local env = sandbox.env.env or {}
    setmetatable(env, nil)

    for k,v in pairs(sandbox.env.base_env) do
        if type(v) == "table" then
            rawset(env, k, sandbox.utils.deepcopy(v))
        else
            rawset(env, k, v)
        end
    end

    rawset(env, "_G", env)

    return env
end
