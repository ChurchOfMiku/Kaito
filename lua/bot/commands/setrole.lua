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
        print(args.user, args.role)
        bot.set_role(args.user, args.role)
    end,
    role = "root",
})
