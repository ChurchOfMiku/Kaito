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
        local user = bot.find_user(msg.service, args.user):await()

        if not user then
            msg:reply("error: no user was found")
        end

        if not bot.has_role_or_higher(user.role, msg.role, true) then
            msg:reply("error: cannot restrict someone with a higher role")
        end

        if user.restricted then
            bot.unrestrict_user(user)
            msg:reply("unrestricted " .. user.name)
        else
            bot.restrict_user(user, msg.user_id):await()
            msg:reply("restricted " .. user.name)
        end
    end,
    role = "admin",
})
