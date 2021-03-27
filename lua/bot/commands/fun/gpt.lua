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
    callback = function(ctx)
        local input = (ctx.args.input or "")

        if #ctx.extra_args > 0 then
            input = input .. " " .. table.concat(ctx.extra_args, " ")
        end

        ctx.msg.channel:send_typing()

        local res = http.fetch("http://127.0.0.1:3000/gpt", { body = input }):await()
        return ctx.msg:reply(ctx.msg.channel:escape_text(res.body)):await()
    end,
})
