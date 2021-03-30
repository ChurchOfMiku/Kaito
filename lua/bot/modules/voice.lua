bot.voice = bot.voice or {connections = {}}

local IDLE_LEAVE_TIME = 60 * 4

local Media = {}

function Media:artist_prefix()
    return self.artist ~= "" and (self.artist.. " - ") or ""
end

function Media:serialize()
    return {
        position = self.position,
        title = self.title,
        artist = self.artist,
        url = self.url,
        media_url = self.media_url
    }
end

function bot.voice.create_media(msg, input)
    local media = {}

    local succ, ytdl_metadata = pcall(function()
        return json.decode(voice.ytdl_metadata(input):await())
    end)
    if not succ then return nil, msg:reply("error: unable to get metadata"):await() end

    media.title = ytdl_metadata.title or input
    media.artist = ytdl_metadata.artist or ytdl_metadata.uploader or ""
    media.url = input
    media.media_url = input
    media.position = 0

    setmetatable(media, { __index = Media })

    return media
end

local VoiceConnection  = {}

function VoiceConnection:queue_media(msg, input)
    self.text_channel = msg.channel

    local media, err_msg = bot.voice.create_media(msg, input)
    if err_msg then return nil, err_msg end

    local has_queue = #self.queue ~= 0 or self.playing
    table.insert(self.queue, media)

    return media, nil, has_queue
end

function VoiceConnection:stop()
    self.conn:stop():await()
end

function VoiceConnection:voteskip(msg)
    self.text_channel = msg.channel

    self.voteskips = self.voteskips or {}

    if self.voteskips[msg.author.id] then
        self.voteskips[msg.author.id] = nil
        return msg:reply(bot.bold_itallic_block(msg.channel, "Voteskip removed: ") .. " (" .. table.count(self.voteskips) .. "/" .. self.voteskips_needed .. ")"):await()
    else
        self.voteskips[msg.author.id] = true

        if not self:voteskip_think() then
            return msg:reply(bot.bold_itallic_block(msg.channel, "Voteskip: ") .. " (" .. table.count(self.voteskips) .. "/" .. self.voteskips_needed .. ")"):await()
        end
    end
end

function VoiceConnection:voteskip_think()
    if self.voteskips then
        local needed = math.ceil(#self.listeners / 3 * 2)
        self.voteskips_needed = needed

        -- Remove the votes from people who leaves
        for k,v in pairs(self.voteskips) do
            if not table.contains(self.listeners, k) then
                self.voteskips[k] = nil
            end
        end

        if table.count(self.voteskips) >= needed then
            self:stop()
            self.text_channel:send(bot.bold_itallic_block(self.text_channel, "The media has been skipped!"))

            return true
        end
    end
end

function VoiceConnection:think()
    local playing = self.conn:playing():await()

    -- Quit if disconnected
    if not self.conn:connected():await() then
        return true
    end

    if not playing then
        self.voteskips = nil
        local next = table.remove(self.queue, 1)
        if next then
            self.playing = next
            self.idle_time = nil
            
            local conn = self.conn
            -- Avoid the entire thing exploding if ytdl shits itself
            pcall(function()
                conn:play(next.media_url):await()
            end)

            self.text_channel:send("🎵 " .. bot.bold_itallic_block(self.text_channel, "Now playing: ") .. self.text_channel:escape_text(next:artist_prefix() .. next.title)):await()
        else
            self.playing = nil

            if self.idle_time then
                if os.time() - self.idle_time > IDLE_LEAVE_TIME then
                    self.conn:disconnect()
                    return true
                end
            else
                self.idle_time = os.time()
            end
        end
    else
        local listeners = self.conn:listeners():await()
        self.listeners = listeners

        self:voteskip_think()
    end
end

function VoiceConnection:idle()
    return not self.conn:playing():await() and #self.queue == 0
end

function VoiceConnection:serialize()
    local playing
    local queue = {}
    
    if self.playing then
        playing = self.playing:serialize()
        playing.position = playing.position + self.conn:position():await()
    end

    for _, media in pairs(self.queue) do
        table.insert(queue, media:serialize())
    end

    return {
        server_id = self.conn.server_id,
        channel_id = self.conn.channel_id,
        text_channel_id = self.text_channel.id,
        playing = playing,
        queue = queue,
        voteskips = self.voteskips
    }
end

function bot.voice.create_connection(server_id, channel_id, text_channel)
    local voice_conn = {}

    voice_conn.conn = voice.join(server_id, channel_id):await()
    voice_conn.text_channel = text_channel
    voice_conn.queue = {}
    voice_conn.listeners = {}

    setmetatable(voice_conn, { __index = VoiceConnection })

    voice_conn.thread = async.spawn(function()
        while not voice_conn:think() do
            async.delay(0.5):await()
        end
    end)

    table.insert(bot.voice.connections, voice_conn)

    return voice_conn
end

function bot.voice.get_connection(msg, only_check)
    if not msg.channel:supports_feature(bot.FEATURES.Voice) then
        return nil, msg:reply("error: service does not support voice"):await()
    end

    local user_channel = voice.user_channel(msg.channel.server, msg.author):await()
    if not user_channel then
        return nil, msg:reply("error: you must be in a voice channel to use music commands"):await()
    end
    local server_id = msg.channel.server.id

    local conn = bot.voice.connection_for_channel(user_channel)
    if conn then return conn end

    local server_conn = bot.voice.connection_for_server(server_id)
    if server_conn and not server_conn:idle() then
        return nil, msg:reply("error: there is already an active voice connection for the server"):await()
    end

    if not only_check then
        return bot.voice.create_connection(server_id, user_channel, msg.channel)
    end
end

function bot.voice.connection_for_channel(channel_id)
    for _,voice in pairs(bot.voice.connections) do
        if voice.conn.channel_id == channel_id then
            return voice
        end
    end
end

function bot.voice.connection_for_server(server_id)
    for _,voice in pairs(bot.voice.connections) do
        if voice.conn.server_id == server_id then
            return voice
        end
    end
end

hooks.add("think", "voice", function()
    for k,v in pairs(bot.voice.connections) do
        if not v.thread or coroutine.status(v.thread) ~= "suspended" then
            bot.voice.connections[k] = nil
            return
        end
    end
end)

local function deserialize_media(media)
    setmetatable(media, { __index = Media })
    return media
end

hooks.add("loaded", "voice", function()
    local data = bot.get_data("voice"):await()
    if not data then return end

    for _,voice_conn in pairs(json.decode(data)) do
        async.spawn(function()
            voice_conn.text_channel = bot.channel(voice_conn.text_channel_id):await()
            voice_conn.conn = voice.join(voice_conn.server_id, voice_conn.channel_id):await()
            voice_conn.listeners = {}

            if voice_conn.playing then
                local position = voice_conn.playing.position
                voice_conn.playing = deserialize_media(voice_conn.playing)

                if position then
                    voice_conn.playing.position = position
                end

                pcall(function()
                    voice_conn.conn:play(voice_conn.playing.media_url, position):await()
                end)    
            end

            for k,v in pairs(voice_conn.queue) do
                voice_conn.queue[k] = deserialize_media(v)
            end

            setmetatable(voice_conn, { __index = VoiceConnection })
    
            voice_conn.thread = async.spawn(function()
                async.delay(0.5):await()

                while not voice_conn:think() do
                    async.delay(0.5):await()
                end

            end)

            table.insert(bot.voice.connections, voice_conn)
        end)
    end
end)

hooks.add("shutdown", "voice", function()
    local data = {}

    for _,conn in pairs(bot.voice.connections) do
        table.insert(data, conn:serialize())
        if conn.text_channel then
            conn.text_channel:send("bot is restarting, expect a small disruption"):await()
        end
    end

    bot.set_data("voice", json.encode(data)):await()
end)
