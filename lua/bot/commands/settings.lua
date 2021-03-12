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
            callback = function(msg, args)
                local module_settings = bot.list_settings(args.module)

                if not module_settings then
                    return msg:reply("unknown module")
                end

                local out = "Module settings:\n"

                local min_len = 0
                for _, v in ipairs(module_settings) do
                    min_len = math.max(min_len, #v.name)
                end
        
                local pad = min_len + 3
                for _, v in ipairs(module_settings) do
                    out =
                        out .. "   " .. bot.icode_block(msg.channel, v.name .. string.rep(" ", pad - #v.name) .. v.help) .. "\n"
                end

                msg:reply(out)
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
            callback = function(msg, args)
                if not (args.server or args.channel) then
                    return msg:reply("argument error: --channel or --server has to be used")
                end

                if args.server and args.channel then
                    return msg:reply("argument error: only one of --channel or --server can be used")
                end

                local server = args.server ~= nil

                local err, fut = bot.set_setting(msg, server, args.module, args.setting, args.value)

                if err then
                    return msg:reply("argument error: " .. err)
                end

                fut:thence(function()
                    msg:reply("Successfully updated \"" .. args.module .. "/" .. args.setting .. "\" for the current " .. (server and "server" or "channel"))
                end):catch(function(err)
                    msg:reply(err)
                end)
            end,
        })
    },
    role = "admin",
})
