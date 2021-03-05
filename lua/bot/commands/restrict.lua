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
    callback = function(msg, args)
        local user = bot.find_user(msg.channel, args.user):await()


        if msg.author.uid == user.uid then
            return msg:reply("error: cannot restrict yourself")
        end

        if not user then
            return msg:reply("error: no user was found")
        end

        if not bot.has_role_or_higher(user.role, msg.author.role, true) then
            return msg:reply("error: cannot restrict someone with a higher role")
        end

        if user.restricted then
            bot.unrestrict_user(user)
            msg:reply("unrestricted " .. user.name)
        else
            bot.restrict_user(user, msg.author):await()
            msg:reply("restricted " .. user.name)
        end
    end,
    role = "admin",
})
