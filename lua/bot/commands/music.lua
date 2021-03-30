bot.add_command("music", {
    description = "Listen to some music",
    aliases = { "play" },
    args = {
        {
            key = "input",
            name = "INPUT",
            description = "input for the music",
            required = true,
        }
    },
    callback = function(ctx)
        local input = ctx.args.input

        local connection, err_msg = bot.voice.get_connection(ctx.msg)
        if err_msg then return err_msg end

        ctx.msg.channel:send_typing()
        
        local media, err_msg, queued = connection:queue_media(ctx.msg, input)
        if err_msg then return err_msg end

        if queued then
            return ctx.msg:reply(bot.bold_itallic_block(ctx.msg.channel, "Queued: ") .. ctx.msg.channel:escape_text(media:artist_prefix() .. media.title)):await()
        end
    end,
    sub_commands = {
        bot.sub_command("skip", {
            args = {
                {
                    key = "force",
                    long = "force",
                    description = "Force (admin)",
                }
            },
            description = "Skip the playing media",
            callback = function(ctx)
                local connection, err_msg = bot.voice.get_connection(ctx.msg)
                if err_msg then return err_msg end

                local force = ctx.args.force
                if force and not bot.has_role_or_higher("admin", ctx.msg.author.role) then
                    return ctx.msg:reply("error: access denied"):await()
                end

                if force then
                    connection:stop()
                    return ctx.msg:reply("The media has been skipped"):await()
                else
                    return connection:voteskip(ctx.msg)
                end
            end,
        }),
        bot.sub_command("queue", {
            description = "Check the music queue",
            args = {
                {
                    key = "page",
                    name = "PAGE",
                    description = "Optional page number",
                },
            },
            callback = function(ctx)
                local connection, err_msg = bot.voice.get_connection(ctx.msg)
                if err_msg then return err_msg end

                local queue = connection.queue

                if #queue == 0 then
                    return ctx.msg:reply("error: the queue is empty"):await()
                else
                    return pagination.create(ctx.msg.channel, {
                        title = "Media queue",
                        data = queue,
                        render_data = function(ctx, media)
                            local content = ""
            
                            for i,media in pairs(media) do
                                if content ~= "" then content = content .. "\n" end
            
                                content = content .. bot.bold_block(ctx.channel, i .. ". ") .. media:artist_prefix() .. media.title
                            end
            
                            return {
                                content = content
                            }
                        end,
                        page = ctx.args.page,
                        caller = ctx.msg.author
                    })
                end
            end,
        }),
        bot.sub_command("leave", {
            description = "Leave the voice channel",
            args = {
                {
                    key = "force",
                    long = "force",
                    description = "Force (admin)",
                }
            },
            callback = function(ctx)
                local connection, err_msg = bot.voice.get_connection(ctx.msg, true)
                if err_msg then return err_msg end
                if not connection then
                    return ctx.msg:reply("error: no active voice connection was found"):await()
                end

                if not ctx.args.force or not bot.has_role_or_higher("admin", ctx.msg.author.role) then
                    if not connection:idle() then
                        return ctx.msg:reply("error: the voice connection is still in use"):await()
                    end
                end

                connection.conn:disconnect():await()
            end,
        }),
        --[[bot.sub_command("volume", {
            description = "Set the music volume",
            args = {
                {
                    key = "volume",
                    name = "VOLUME",
                    description = "Volume to set",
                    required = true,
                }
            },
            role = "admin",
            callback = function(ctx)
                local connection, err_msg = bot.voice.get_connection(ctx.msg, true)
                if err_msg then return err_msg end
                if not connection then
                    return ctx.msg:reply("error: no active voice connection was found"):await()
                end

                local volume = tonumber(ctx.args.volume)

                if not volume or 0 > volume then
                    return ctx.msg:reply("error: the volume must be a number over 0"):await()
                end

                connection.conn:disconnect():await()
            end,
        }),]]

    }
})
