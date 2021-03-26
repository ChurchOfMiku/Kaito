bot.add_command("gpt", {
    description = "Generate a random sentence using GPT",
    aliases = { "g" },
    args = {
        {
            key = "input",
            name = "INPUT",
            description = "Input for start of sentence",
            required = true,
        }
    },
    callback = function(msg, args, extra_args)
        local input = (args.input or "")

        if #extra_args > 0 then
            input = input .. " " .. table.concat(extra_args, " ")
        end

        msg.channel:send_typing()

        local res = http.fetch("http://127.0.0.1:3000/gpt", { body = input }):await()
        return msg:reply(msg.channel:escape_text(res.body)):await()
    end,
})
