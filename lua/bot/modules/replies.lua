local function count_caps(str)
    local _, caps = string.gsub(str, "[A-Z]", "")
    return caps
end

local function count_chars(str)
    local _, chars = string.gsub(str, "[A-Za-z]", "")
    return chars
end

local REPLIES = {
    ["thanks kaito"] = { "You're welcome", "No problem!", "Nya~!" },
    ["based kaito"] = {"😎", "💪", "Nya~!"},
    ["fuck kaito"] = {"Yes, uwu", "Yes, fuck me", "Nya~!!!", "Myaa~!!"}
}

hooks.add("message", "replies", function(msg)
    for check, replies in pairs(REPLIES) do
        local dist = string.levenshtein(string.sub(string.lower(msg.content), 1, #check), check)

        if dist < 3 then
            local reply = replies[math.random(1, #replies)]
            local reply_chars = count_chars(reply)

            local caps = math.floor(count_caps(msg.content) * (reply_chars / count_chars(msg.content)) + 0.5)

            if caps == reply_chars then
                reply = string.upper(reply)
            elseif caps > 2 then
                reply = string.lower(reply)

                for i=1, caps do
                    for _=1, 3 do
                        local offset = math.random(1, #reply)
                        local c = string.sub(reply, offset, offset)
                        if string.match(c, "[a-z]") then
                            reply = string.sub(reply, 1, offset - 1) .. string.upper(c) .. string.sub(reply, offset + 1, #reply)

                            break
                        end
                    end
                end
            elseif caps == 0 then
                reply = string.lower(reply)
            end

            for i=1,dist do
                local offset = math.random(1, #reply)
                reply = string.sub(reply, 1, offset) .. string.sub(reply, offset + 2, #reply)
            end

            msg:reply(reply)
        end
    end
end)
