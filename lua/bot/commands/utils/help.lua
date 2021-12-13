bot.add_command("help", {
    description = "Information on how to use the bot",
    args = {
        {
            key = "page",
            name = "PAGE",
            description = "Optional page number",
        },
    },
    callback = function(ctx)
        local cmds = {}

        for _,cmd in pairs(bot.cmds) do
            if cmd.role then
                if bot.has_role_or_higher(cmd.role, ctx.msg.author.role) then
                    table.insert(cmds, cmd)
                end
            else
                table.insert(cmds, cmd)
            end
        end

        table.sort(cmds, function(a, b) return a.cmd < b.cmd end)

        return pagination.create(ctx.msg.channel, {
            title = "Kaito - Help",
            data = cmds,
            render_data = function(ctx, cmds)
                local content = ""

                for _,cmd in pairs(cmds) do
                    if content ~= "" then content = content .. "\n" end

                    content = content .. bot.bold_block(ctx.channel, cmd.cmd) .. "\n" .. cmd.description
                end

                return {
                    content = content
                }
            end,
            pages = {
                function(ctx)
                    return {
                        content = string.trim_lines([[
                            (ﾉ> ◇ <)ﾉ♪♪♪
                            
                            In short Kaito uses shell style command parsing where spaces seperates the arguments in a command.
                            
                            The help command can be navigated by using the page number as the first argument passed to the command, or by using reactions if the service supports it.
                            Additionally, the --help flag may be used on any command to see its subcommands, options and arguments.]])
                    }
                end
            },
            page = ctx.args.page,
            caller = ctx.msg.author
        })
    end,
})
