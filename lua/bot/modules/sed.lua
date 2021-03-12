sed = sed or {}
sed.channels = sed.channels or {}

hooks.add("message", "sed", function(msg)
    local channel_buffer = sed.channels[msg.channel.id]

    if not channel_buffer then
        sed.channels[msg.channel.id] = RingBuffer(12)
        channel_buffer = sed.channels[msg.channel.id]
    end

    local find, replace = string.match(msg.content, "^sed/(.*)/(.*)/$")

    if find then
        for i=channel_buffer:get_size(), 1, -1  do
            local prev_msg = channel_buffer:get(i)
            
            if string.find(prev_msg.content, find) then
                local replaced = string.gsub(prev_msg.content, find, replace)
                msg:reply(msg.channel:escape_text(prev_msg.author.nick) .. ": " .. msg.channel:escape_text(replaced))

                break
            end
        end
    else
        channel_buffer:push(msg)
    end
end)
