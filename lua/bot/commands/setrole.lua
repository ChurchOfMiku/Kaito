bot.add_command("setrole", {
    description = "Set the role of a user",
    args = {
        {
            key = "user",
            name = "USER",
            description = "User to update the role",
            required = true,
        },
        {
            key = "role",
            name = "ROLE",
            description = "Role to be changed to",
            required = true,
        }
    },
    callback = function(msg, args)
        local user = bot.find_user(msg.channel, args.user):await()

        if user then
            bot.set_role(user, args.role):await()
            return msg:reply("changed role of " .. user.name .. " to "..args.role):await()
        else
            return msg:reply("error: no user was found"):await()
        end
    end,
    role = "root",
})
