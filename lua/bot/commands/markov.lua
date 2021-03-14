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
        if running >= 3 then
            return msg:reply("current running markov operations limit reached")
        end

        running = running + 1

        local status, err = pcall(function()
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

                    return
                end
    
                if reply then
                    reply:edit(msg.channel:escape_text(body.body))
                else
                    reply = msg.channel:send(msg.channel:escape_text(body.body)):await()
                end
            end
        end)

        running = math.max(running - 1, 0)

        if err then
            error(err)
        end
    end,
})
