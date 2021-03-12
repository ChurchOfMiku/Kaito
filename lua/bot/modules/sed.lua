local buffer = RingBuffer(12)

hooks.add("message", "sed", function(msg)
    local find, replace = string.match(msg.content, "^sed/(.*)/(.*)/$")

    if find then
        for i=buffer:get_size(), 1, -1  do
            local prev_msg = buffer:get(i)
            
            if string.find(prev_msg.content, find) then
                local replaced = string.gsub(prev_msg.content, find, replace)
                msg:reply(msg.channel:escape_text(prev_msg.author.nick) .. ": " .. msg.channel:escape_text(replaced))

                break
            end
        end
    else
        buffer:push(msg)
    end
end)
