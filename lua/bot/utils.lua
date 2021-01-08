bot.utils = bot.utils or {}

bot.utils.array_has_value = function(tbl, value)
    for k,v in ipairs(tbl) do
        if v == value then
            return true
        end
    end

    return false
end