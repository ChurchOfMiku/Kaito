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
    callback = function(ctx)
        local user = bot.find_user(ctx.msg.channel, ctx.args.user):await()

        if user then
            bot.set_role(user, ctx.args.role):await()
            return ctx.msg:reply("changed role of " .. user.name .. " to "..ctx.args.role):await()
        else
            return ctx.msg:reply("error: no user was found"):await()
        end
    end,
    role = "root",
})
