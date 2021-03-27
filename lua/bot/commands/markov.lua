local running = 0

bot.add_command("markov", {
    description = "Generate a random sentence using Markov",
    aliases = { "m" },
    args = {
        {
            key = "input",
            name = "INPUT",
            description = "Input for start of sentence",
        }
    },
    callback = function(msg, args, extra_args)
        if running >= 6 then
            return msg:reply("current running markov operations limit reached"):await()
        end

        running = running + 1

        local succ, res = pcall(function()
            local input = (args.input or "")

            if #extra_args > 0 then
                input = input .. " " .. table.concat(extra_args, " ")
            end
    
            msg.channel:send_typing()
    
            local res = http.fetch("http://127.0.0.1:3000/markov", { body = input, stream = true }):await()
            local reply
            local body = res
    
            while body.next_body do
                body = body.next_body:await()
    
                if not body then
                    if reply and msg.channel:supports_feature(bot.FEATURES.React) then
                        reply:react("✅")
                    end

                    return reply
                end
    
                if reply then
                    reply:edit(msg.channel:escape_text(body.body)):await()
                else
                    reply = msg.channel:send(msg.channel:escape_text(body.body)):await()
                    bot.add_command_history(msg, reply)
                end
            end

            return reply
        end)

        running = math.max(running - 1, 0)

        if succ then
            return res
        -- Throw the error if it was not due to the message being deleted
        elseif string.match(res, "Unknown Message") then
            error(res)
        end
    end,
})
