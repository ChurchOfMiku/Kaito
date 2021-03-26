pagination = pagination or {}
pagination.DEFAULT_PER_PAGE = 8
pagination.INTERACTIVE_TIME = 480
pagination.EMOJI_LEFT_ARROW = "⬅"
pagination.EMOJI_RIGHT_ARROW = "➡"
pagination.EMOJI_CROSS = "❌"

function pagination.create(channel, options)
    local can_edit = channel:supports_feature(bot.FEATURES.Edit)
    local can_react = channel:supports_feature(bot.FEATURES.React)

    local interactive = can_edit and can_react

    local num_pages = (options.pages and #options.pages or 0)
    local num_data = (options.data and #options.data or 0)
    local per_page = options.per_page or pagination.DEFAULT_PER_PAGE

    local tot_pages = num_pages

    if options.data then
        tot_pages = tot_pages + math.ceil(#options.data / per_page)
    end

    local start_page = math.min(math.max(options.page and tonumber(options.page) or 1, 1), tot_pages)
    local ctx = {channel = channel, page_num = start_page}


    local create_content = function()
        local page_num = ctx.page_num
        local page = nil

        if page_num > num_pages or num_pages == 0 then
            local data_page = page_num - num_pages - 1
            local offset = (data_page * per_page) + 1
            local data_end = offset + per_page - 1

            ctx.offset = offset

            local data = {table.unpack(options.data, offset, data_end)}
    
            page = options.render_data(ctx, data)
        else
            page = options.pages[page_num](ctx)
        end

        return (options.title and options.title .. "\n" or "") .. page.content .. "\nPage "..ctx.page_num.."/"..tot_pages
    end

    local msg = channel:send(create_content()):await()

    if interactive and msg then
        bot.reaction_hooks[msg.id] = function(msg, reactor, reaction, removed)
            if reactor.uid == options.caller.uid or bot.has_role_or_higher("admin", reactor.role) then
                if reaction == pagination.EMOJI_LEFT_ARROW or reaction == pagination.EMOJI_RIGHT_ARROW then
                    local offset = reaction == pagination.EMOJI_RIGHT_ARROW and 1 or -1
                    ctx.page_num = math.min(math.max(ctx.page_num + offset, 1), tot_pages)
                    msg:edit(create_content()):await()
                elseif reaction == pagination.EMOJI_CROSS then
                    msg:delete():await()
                end
            end
        end

        msg:react(pagination.EMOJI_LEFT_ARROW):await()
        msg:react(pagination.EMOJI_RIGHT_ARROW):await()
        msg:react(pagination.EMOJI_CROSS):await()

        async.delay(pagination.INTERACTIVE_TIME):await()

        bot.reaction_hooks[msg.id] = nil
    end

    return msg
end
