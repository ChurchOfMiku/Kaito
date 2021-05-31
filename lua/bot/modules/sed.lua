hooks.add("message", "sed", function(msg)
    local channel_buffer = bot.cache.messages[msg.channel.id]

    if not channel_buffer then
        return
    end

    local find_g, replace = string.match(msg.content, "^sed/(.-)/(.-)/g$")
    local find, replace2 = string.match(msg.content, "^sed/(.-)/(.-)/?$")
    local replace = replace or replace2

    if find_g or find then
        for i=channel_buffer:get_size(), 1, -1  do
            local prev_msg = channel_buffer:get(i)
            
            if string.find(prev_msg.content, find_g or find) then
                local replaced = find_g and string.gsub(prev_msg.content, find_g, replace) or string.gsub(prev_msg.content, find, replace, 1)
                msg:reply(msg.channel:escape_text(prev_msg.author.nick) .. ": " .. msg.channel:escape_text(replaced))

                break
            end
        end
    end
end)
