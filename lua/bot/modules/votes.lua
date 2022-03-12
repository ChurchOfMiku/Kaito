local VOTE_EMOJIS = { "1️⃣", "2️⃣", "3️⃣", "4️⃣", "5️⃣", "6️⃣", "7️⃣", "8️⃣", "9️⃣", "🔟" }

bot.votes = bot.votes or {active_votes = {}, last_vote = {}}

local Vote  = {}

function Vote:time_text()
    if self.ended then
        return "The vote has ended"
    else
        local time_left = self.end_time - os.time()

        if time_left <= 0 then
            return nil
        elseif time_left >= 60 * 60 then
            local hours = math.ceil(time_left / (60 * 60))
            return "The vote is ending  in " .. hours ..  " hour" .. string.plural(hours)
        elseif time_left >= 30 then
            local minutes = math.ceil(time_left / 60)
            return "The vote is ending  in " .. minutes ..  " minute" .. string.plural(minutes)
        elseif time_left <= 10 then
            return "The vote is ending in 10 seconds"
        else
            return "The vote is ending in 30 seconds"
        end
    end
end

function Vote:msg_text(full)
    local time_text 
    if not full then
        time_text = self:time_text()
        if not time_text or time_text == self.last_time_text then return end
        self.last_time_text = time_text
    end

    local text = bot.bold_block(self.channel, self.ended and "Vote results:" or "Vote:") .. " " .. self.title .. "\n\n"

    for i, option in pairs(self.options) do
        if self.ended and self.results then
            text = text .. bot.bold_block(self.channel, i .. ". ") .. option .. ": " .. #self.results[i] ..  "/".. self.total_votes .. "\n"

            if full and #self.results[i] > 0 then
                text = text .. bot.code_block(self.channel, table.list_words(table.map(self.results[i], function(user_id)
                    return bot.find_user(self.channel, user_id):await().name
                end))) .. "\n"
            end
        else
            text = text .. bot.bold_block(self.channel, i .. ". ") .. option .. "\n"
        end
    end

    if self.ended and self.results then
        local highest = 0
        local results = {}

        for i = 1, #self.results do
            local votes = #self.results[i]
            if votes > 0 then
                results[votes] = results[votes] or {}
                table.insert(results[votes], self.options[i])

                highest = math.max(highest, votes)
            end
        end

        if highest > 0 then
            text = text .. bot.bold_block(self.channel, "Winner: ") .. table.list_words(results[highest]) .. "!\n"
        end
    end

    if not full then
        text = text .. "\n" ..  time_text
    end

    return text
end

function Vote:think()
    if self.ended then return true end

    if self:should_end() then
        self:end_vote()
        return true
    end

    local updated_text = self:msg_text()
    if not updated_text then return end

    self.msg:edit(updated_text)
end

function Vote:on_reaction(msg, reactor, reaction, removed)
    if msg.author.id == reactor.id then return end

    local i = table.contains(VOTE_EMOJIS, reaction)
    if not i then return end
    
    if removed then
        if self.votes[reactor.id] == i then
            self.votes[reactor.id] = nil
        end
    else
        self.votes[reactor.id] = i
    end
end

function Vote:should_end()
    return os.time() > self.end_time
end

function Vote:end_vote()
    self.ended = true
    bot.reaction_hooks[self.msg.id] = nil

    -- Create the results table
    local results = {}
    for i = 1, #self.options do
        results[i] = {}
    end

    local user_votes = {}

    for k,v in pairs(self.votes) do
        user_votes[k] = v
    end

    local total_votes = 0

    -- Turn the user votes into results
    for k,v in pairs(user_votes) do
        if v <= #self.options then
            table.insert(results[v] , k)
            total_votes = total_votes + 1
        end
    end

    self.total_votes = total_votes
    self.results = results
    bot.votes.last_vote[self.channel.id] = self

    self.msg:edit(self:msg_text())
end



function Vote:vote(user, choice)
    if choice then
        if choice < 1 or choice > #self.options then
            return "choice out of range"
        end
    
        self.votes[user.id] = choice
        return "your choice has been set to " .. choice
    else
        self.votes[user.id] = nil
        return "your choice has been removed"
    end
end

function Vote:serialize()
    return {
        title = self.title,
        author = self.author,
        duration = self.duration,
        end_time = self.end_time,
        options = self.options,
        channel_id = self.channel.id,
        message_id = self.msg.id,
        interactive = self.interactive,
        votes = self.votes,
        ended = self.ended,
        results = self.results
    }
end

function Vote:set_time(time)
    self.duration = math.max(math.min(time, 60 * 60 *2), 10)
    self.end_time = os.time() + self.duration
end

function bot.votes.get_vote_for_channel(channel)
    for k,v in pairs(bot.votes.active_votes) do
        if v.channel.id == channel.id then
            return v
        end
    end
end

function bot.votes.create(author, channel, title, time, options)
    if time == 0 then return channel:send("error: invalid time spesified") end
    if not channel:supports_feature(bot.FEATURES.Edit) then
        return channel:send("error: channel does not support message editing which is required for votes")
    end

    if #options == 0 then
        return channel:send("at least one option is required")
    end

    if #options > #VOTE_EMOJIS then
        return channel:send("error: maximum amount of vote options is " .. #VOTE_EMOJIS)
    end

    if bot.votes.get_vote_for_channel(channel) then
        return channel:send("error: there is already an active vote for the channel")
    end

    local vote = {}

    vote.author = author.id
    vote.title = title
    vote.duration = math.max(math.min(time, 60 * 60 *2), 10)
    vote.end_time = os.time() + vote.duration
    vote.options = options
    vote.channel = channel
    vote.interactive = channel:supports_feature(bot.FEATURES.React)
    vote.votes = {}

    for k,v in pairs(bot.votes.active_votes) do
        if v.channel.id == channel.id then
            return channel:send("error: there is already an active vote in the current channel")
        end
    end

    setmetatable(vote, { __index = Vote })

    vote.msg = channel:send(vote:msg_text()):await()

    vote.thread = async.spawn(function()
        while not vote:think() do
            async.delay(1):await()
        end
    end)

    if vote.interactive then
        async.spawn(function()
            for i= 1, #options do
                vote.msg:react(VOTE_EMOJIS[i]):await()
            end
        end)

        bot.reaction_hooks[vote.msg.id] = function(msg, reactor, reaction, removed)
            vote:on_reaction(msg, reactor, reaction, removed)
        end
    end

    table.insert(bot.votes.active_votes, vote)

    return vote
end

hooks.add("loaded", "votes", function()
    local data = bot.get_data("votes"):await()
    if not data then return end

    for _,vote in pairs(json.decode(data)) do
        async.spawn(function()
            vote.channel = bot.channel(vote.channel_id):await()
            vote.msg = bot.message(vote.channel_id, vote.message_id):await()
            setmetatable(vote, { __index = Vote })
    
            vote.thread = async.spawn(function()
                while not vote:think() do
                    async.delay(1):await()
                end
            end)

            if not vote.ended and vote.interactive then
                bot.reaction_hooks[vote.msg.id] = function(msg, reactor, reaction, removed)
                    vote:on_reaction(msg, reactor, reaction, removed)
                end
            end
        end)
    end
end)

hooks.add("shutdown", "votes", function()
    local data = {}

    for _,vote in pairs(bot.votes.active_votes) do
        table.insert(data, vote:serialize())
    end

    bot.set_data("votes", json.encode(data)):await()
end)


hooks.add("think", "votes", function()
    for k,v in pairs(bot.votes.active_votes) do
        if not v.thread or coroutine.status(v.thread) ~= "suspended" then
            bot.votes.active_votes[k] = nil
            return
        end
    end
end)
