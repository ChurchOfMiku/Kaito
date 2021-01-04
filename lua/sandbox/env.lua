sandbox.env = sandbox.env or {}

sandbox.env.base_env = {
    _VERSION = _VERSION
}

sandbox.env.get_env = function()
    local env = sandbox.env.env or {}

    for k,v in pairs(sandbox.env.base_env) do
        env[k] = v
    end

    return env
end
