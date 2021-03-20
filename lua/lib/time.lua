time = time or {}

local function add_time(time, letter, times)
    local secs = 0

    local res = string.match(time, "%d+"..letter)

    if res then
        secs = secs + tonumber(res:sub(1,#res - 1)) * times
    end

    return secs
end

function time.parse_duration(time)
    return add_time(time, "s", 1)
        + add_time(time, "m", 60)
        + add_time(time, "h", 60 * 60)
        + add_time(time, "d", 60 * 60 * 24)
        + add_time(time, "w", 60 * 60 * 24 * 7)
        + math.floor(add_time(time, "M", 60 * 60 * 24 * (365 / 12)))
        + add_time(time, "Y", 60 * 60 * 24 * 365)
end
