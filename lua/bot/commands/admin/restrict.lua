bot.add_command("restrict", {
    description = "Set the role of a user",
    args = {
        {
            key = "user",
            name = "USER",
            description = "User to restrict or unrestrict",
            required = true,
        }
    },
    callback = function(ctx)
        local user = bot.find_user(ctx.msg.channel, ctx.args.user):await()


        if ctx.msg.author.uid == user.uid then
            return ctx.msg:reply("error: cannot restrict yourself"):await()
        end

        if not user then
            return ctx.msg:reply("error: no user was found"):await()
        end

        if not bot.has_role_or_higher(user.role, ctx.msg.author.role, true) then
            return ctx.msg:reply("error: cannot restrict someone with a higher role"):await()
        end

        if user.restricted then
            bot.unrestrict_user(user)
            return ctx.msg:reply("unrestricted " .. user.name):await()
        else
            bot.restrict_user(user, ctx.msg.author):await()
            return ctx.msg:reply("restricted " .. user.name):await()
        end
    end,
    role = "admin",
})
