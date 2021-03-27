bot.add_command("vote", {
    description = "Cast your vote",
    args = {
        {
            key = "choice",
            name = "CHOICE",
            description = "Choice (number) or none for removing choice",
        }
    },
    callback = function(ctx)
        local vote = bot.votes.get_vote_for_channel(ctx.msg.channel)
        if not vote then
            return ctx.msg:reply("error: no active vote was found for the current channel"):await()
        end

        local choice = ctx.args.choice and tonumber(ctx.args.choice)

        return ctx.msg:reply(vote:vote(ctx.msg.author, choice)):await()
    end,
    sub_commands = {
        bot.sub_command("create", {
            args = {
                {
                    key = "title",
                    name = "TITLE",
                    description = "TITLE of vote",
                    required = true,
                },
                {
                    key = "time",
                    name = "TIME",
                    description = "Vote time. e.g. 1h, 1m or 1m50s (max 2 hours)",
                    required = true,
                },
                {
                    key = "first_option",
                    name = "OPTIONS",
                    description = "Vote options",
                    required = true,
                }
            },
            description = "Create a new vote in the channel",
            callback = function(ctx)
                local options = ctx.extra_args
                table.insert(options, 1, ctx.args.first_option)

                return bot.votes.create(ctx.msg.author, ctx.msg.channel, ctx.args.title, time.parse_duration(ctx.args.time), options).msg
            end,
            role = "trusted"
        }),
        bot.sub_command("end", {
            description = "End the current vote in the channel",
            aliases = { "abort" },
            callback = function(ctx)
                local vote = bot.votes.get_vote_for_channel(ctx.msg.channel)
                if not vote then
                    return ctx.msg:reply("error: no active vote was found for the current channel"):await()
                end
        
                if not vote.author == ctx.msg.author.id and not bot.has_role_or_higher("admin", ctx.msg.author.role) then
                    return ctx.msg:reply("error: only the creator of the vote or an admin can end the vote"):await()
                end

                vote:end_vote()
            end,
            role = "admin"
        }),
        bot.sub_command("time", {
            description = "Set the time remainding for the current vote",
            args = {
                {
                    key = "time",
                    name = "TIME",
                    description = "time to use",
                }
            },
            callback = function(ctx)
                local vote = bot.votes.get_vote_for_channel(ctx.msg.channel)
                if not vote then
                    return ctx.msg:reply("error: no active vote was found for the current channel"):await()
                end

                local time = time.parse_duration(ctx.args.time)
                if time == 0 then
                    return ctx.msg:reply("error: invalid duration \"" .. ctx.args.time .. "\""):await()
                end

                vote:set_time(time)
            end,
            role = "admin"
        }),
        bot.sub_command("results", {
            description = "Get detailed results from the last vote",
            callback = function(ctx)
                local vote = bot.votes.last_vote[ctx.msg.channel.id]

                if not vote then
                    return ctx.msg:reply("error: no previous vote was found for the current channel"):await()
                end

                return ctx.msg:reply(vote:msg_text(true)):await()
            end,
            role = "trusted"
        })
    }
})
