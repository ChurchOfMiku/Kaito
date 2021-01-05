sandbox.utils = sandbox.utils or {}

sandbox.utils.deepcopy = function(orig)
    local orig_type = type(orig)
    local copy
    if orig_type == "table" then
        copy = {}
        for orig_key, orig_value in next, orig, nil do
            copy[deepcopy(orig_key)] = deepcopy(orig_value)
        end
        setmetatable(copy, deepcopy(getmetatable(orig)))
    else
        copy = orig
    end
    return copy
end

sandbox.utils.setfenv = function(fn, env)
    local i = 1
    while true do
        local name = debug.getupvalue(fn, i)
        if name == "_ENV" then
            debug.upvaluejoin(
                fn,
                i,
                (function()
                    return env
                end),
                1
            )
            break
        elseif not name then
            break
        end

        i = i + 1
    end

    return fn
end

sandbox.utils.getfenv = function(fn)
    local i = 1
    while true do
        local name, val = debug.getupvalue(fn, i)
        if name == "_ENV" then
            return val
        elseif not name then
            break
        end
        i = i + 1
    end
end

sandbox.utils.is_array = function(tbl)
    local i = 0
    for _ in pairs(tbl) do
        i = i + 1
        if tbl[i] == nil then return false end
    end
    return true
end

sandbox.utils.table_to_string = function(tbl, indent, key)
    indent = indent or 0

    local len = 0
    local left_pad = string.rep(" ", indent)

    for _,_ in pairs(tbl) do
        len = len + 1
    end

    if len == 0 then
        return left_pad .. "{}"
    end

    local is_array = sandbox.utils.is_array(tbl)
    local newlines = indent ~= 0 or not is_array

    if is_array and not newlines then
        local total_len = 0
        for k,v in pairs(tbl) do
            local t = type(v)
            if t == "table" then
                newlines = true
                break
            else
                local len = string.len(tostring(v))

                if len > 10 then
                    newlines = true
                    break
                end

                total_len = total_len + len
            end
        end

        newlines = total_len > 400
    end

    local left_pad2 = newlines and string.rep(" ", indent + 2) or ""
    local out = left_pad .. (key and (key .. " = ") or "") .. "{" .. (newlines and "\n" or " ")

    for k,v in pairs(tbl) do
        if type(v) == "table" then
            out = out .. sandbox.utils.table_to_string(v, indent + 2, k)
        else
            out = out .. left_pad2 .. (is_array and "" or k .. " = ") .. tostring(v)
        end

        if next(tbl, k) == nil then
            out = out .. (newlines and "\n" or " ")
        else
            out = out .. "," .. (newlines and "\n" or " ")
        end
    end

    out = out .. left_pad .. "}"

    return out
end
