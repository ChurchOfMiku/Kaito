bot.add_command("settings", {
    description = "Update module settings for the channel or server",
    sub_commands = {
        bot.sub_command("list", {
            args = {
                {
                    key = "module",
                    name = "MODULE",
                    description = "Module to list options for",
                    required = true,
                },
            },
            description = "List the settings for the module",
            callback = function(ctx)
                local module_settings = bot.list_settings(ctx.args.module)

                if not module_settings then
                    return ctx.msg:reply("unknown module")
                end

                local out = "Module settings:\n"

                local min_len = 0
                for _, v in ipairs(module_settings) do
                    min_len = math.max(min_len, #v.name)
                end
        
                local pad = min_len + 3
                for _, v in ipairs(module_settings) do
                    out =
                        out .. "   " .. bot.icode_block(ctx.msg.channel, v.name .. string.rep(" ", pad - #v.name) .. v.help) .. "\n"
                end

                ctx.msg:reply(out)
            end,
        }),
        bot.sub_command("set", {
            args = {
                {
                    key = "module",
                    name = "MODULE",
                    description = "Module for the settings",
                    required = true,
                },
                {
                    key = "setting",
                    name = "SETTING",
                    description = "Setting to update",
                    required = true,
                },
                {
                    key = "value",
                    name = "VALUE",
                    description = "Value for setting",
                    required = true,
                },
                {
                    key = "server",
                    long = "server",
                    description = "apply the setting for the current server"
                },
                {
                    key = "channel",
                    long = "channel",
                    description = "apply the setting for the current channel"
                }
            },
            description = "Update a module setting",
            callback = function(ctx)
                if not (ctx.args.server or ctx.args.channel) then
                    return ctx.msg:reply("argument error: --channel or --server has to be used"):await()
                end

                if ctx.args.server and ctx.args.channel then
                    return ctx.msg:reply("argument error: only one of --channel or --server can be used"):await()
                end

                local server = ctx.args.server ~= nil

                local err, fut = bot.set_setting(ctx.msg, server, ctx.args.module, ctx.args.setting, ctx.args.value)

                if err then
                    return ctx.msg:reply("argument error: " .. err):await()
                end

                fut:await()

                return ctx.msg:reply("Successfully updated \"" .. ctx.args.module .. "/" .. ctx.args.setting .. "\" for the current " .. (server and "server" or "channel")):await()
            end,
        })
    },
    role = "admin",
})
