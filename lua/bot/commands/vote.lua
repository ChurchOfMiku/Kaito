bot.add_command("vote", {
    description = "Cast your vote",
    args = {
        {
            key = "choice",
            name = "CHOICE",
            description = "Choice (number) or none for removing choice",
        }
    },
    callback = function(msg, args)
        local vote = bot.votes.get_vote_for_channel(msg.channel)
        if not vote then
            return msg:reply("error: no active vote was found for the current channel"):await()
        end

        local choice = args.choice and tonumber(args.choice)

        return msg:reply(vote:vote(msg.author, choice)):await()
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
            callback = function(msg, args, extra_args)
                local options = extra_args
                table.insert(options, 1, args.first_option)

                return bot.votes.create(msg.author, msg.channel, args.title, time.parse_duration(args.time), options).msg
            end,
            role = "trusted"
        }),
        bot.sub_command("end", {
            description = "End the current vote in the channel",
            aliases = { "abort" },
            callback = function(msg)
                local vote = bot.votes.get_vote_for_channel(msg.channel)
                if not vote then
                    return msg:reply("error: no active vote was found for the current channel"):await()
                end
        
                if not vote.author == msg.author.id and not bot.has_role_or_higher("admin", msg.author.role) then
                    return msg:reply("error: only the creator of the vote or an admin can end the vote"):await()
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
            callback = function(msg, args)
                local vote = bot.votes.get_vote_for_channel(msg.channel)
                if not vote then
                    return msg:reply("error: no active vote was found for the current channel"):await()
                end

                local time = time.parse_duration(args.time)
                if time == 0 then
                    return msg:reply("error: invalid duration \"" .. args.time .. "\""):await()
                end

                vote:set_time(time)
            end,
            role = "admin"
        }),
        bot.sub_command("results", {
            description = "Get detailed results from the last vote",
            callback = function(msg, args)
                local vote = bot.votes.last_vote[msg.channel.id]

                if not vote then
                    return msg:reply("error: no previous vote was found for the current channel"):await()
                end

                return msg:reply(vote:msg_text(true)):await()
            end,
            role = "trusted"
        })
    }
})
