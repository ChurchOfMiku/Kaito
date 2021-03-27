local MAX_UNDO = 5
local MAX_UNDO_ADMIN = 32

bot.add_command("undo", {
    description = "Undo your last commands",
    args = {
        {
            key = "amount",
            name = "AMOUNT",
            description = "Amount of messages to undo, max: " .. MAX_UNDO,
        },
        {
            key = "user",
            name = "USER",
            description = "User to undo commands for (admin)",
        },
    },
    callback = function(ctx)
        local amount = tonumber(ctx.args.amount)
        local has_amount = amount ~= nil
        amount = math.max(amount or 1, 1)

        local admin = bot.has_role_or_higher("admin", ctx.msg.author.role)
        local user_arg = ctx.args.user or not has_amount and ctx.args.amount

        if user_arg and not admin then
            return ctx.msg:reply("error: only admins can undo messages for other users"):await()
        end

        local user = user_arg and bot.find_user(ctx.msg.channel, user_arg):await()

        if admin then
            amount = math.min(amount, MAX_UNDO_ADMIN)
        else
            amount = math.min(amount, MAX_UNDO)
        end
        
        local channel_buffer = bot.cache.messages[ctx.msg.channel.id]

        local count = 0

        if channel_buffer then
            for i=channel_buffer:get_size(), 1, -1 do
                local old_msg = channel_buffer:get(i)
                local command = bot.cache.commands:get(old_msg.id)

                if user then
                    if old_msg.author.uid == user.uid then
                        if command and bot.delete_reply(command) then
                            count = count + 1
                        end

                        if bot.delete_lua_replies(old_msg.id):await() then
                            count = count + 1
                        end
                    end
                elseif ctx.msg.author.uid == old_msg.author.uid then
                    if command and bot.delete_reply(command) then
                        count = count + 1
                    end

                    if bot.delete_lua_replies(old_msg.id):await() then
                        count = count + 1
                    end
                end

                if count == amount then
                    break
                end
            end
        end

        if count > 0 then
            return ctx.msg:reply("successfully deleted " .. count .. " commands"):await()
        else
            return ctx.msg:reply("could not find any commands to undo"):await()
        end
    end,
})
