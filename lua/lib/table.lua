function table.map(tbl, f)
    local t = {}

    for k,v in pairs(tbl) do
        t[k] = f(v)
    end

    return t
end

function table.contains(tbl, value)
    for k,v in pairs(tbl) do
        if v == value then
            return k
        end
    end

    return false
end

function table.list_words(words)
    local len = #words
    if len == 1 then
        return words[1]
    elseif len == 2 then
        return words[1] .. " and " .. words[2]
    else
        local out = ""
        for i = 1, len do
            if i == len then
                out = out .. words[i]
            elseif i == len -1 then
                out = out .. words[i] .. " and "
            else
                out = out .. words[i] .. ", "
            end
        end

        return out
    end
end
