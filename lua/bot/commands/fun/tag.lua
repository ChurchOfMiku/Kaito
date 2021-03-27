bot.add_command("tag", {
    description = "View a tag",
    aliases = { "t" },
    args = {
        {
            key = "tag",
            name = "NAME",
            description = "Tag name",
            required = true,
        }
    },
    callback = function(ctx)
        local tag = tags.find_tag(ctx.msg.channel.server, ctx.args.tag):await()

        if tag then
            return ctx.msg:reply(ctx.msg.channel:escape_text(tags.exec_tag(ctx.msg, ctx.msg.author, ctx.msg.channel, tag, ctx.extra_args))):await()
        else
            return ctx.msg:reply("error: unknown tag"):await()
        end
    end,
    sub_commands = {
        bot.sub_command("create", {
            args = {
                {
                    key = "tag",
                    name = "NAME",
                    description = "Tag name",
                    required = true,
                },
                {
                    key = "value",
                    name = "VALUE",
                    description = "Tag value",
                }
            },
            description = "Create a new tag",
            callback = function(ctx)
                if not tags.is_valid_name(ctx.args.tag) then
                    return ctx.msg:reply("error: the tag name must be alphanumeric"):await()
                end

                if #ctx.args.tag > tags.MAX_NAME_LIMIT then
                    return ctx.msg:reply("error: the tag name cannot be longer than " .. tags.MAX_NAME_LIMIT .. " characters"):await()
                end

                local value = ctx.args.value or ""

                if #ctx.extra_args > 0 then
                    value = value .. " " .. table.concat(ctx.extra_args, " ")
                end

                for i, attachment in pairs(ctx.msg.attachments) do
                    if value ~= "" then value = value .. "\n" end

                    value = value .. attachment.url
                end

                if #value > tags.MAX_VALUE_LIMIT then
                    return ctx.msg:reply("error: the tag value cannot be longer than " .. tags.MAX_VALUE_LIMIT .. " characters"):await()
                end

                if #value == 0 then
                    return ctx.msg:reply("error: the tag value cannot be empty"):await()
                end

                if tags.count_user_tags(ctx.msg.author):await() > tags.MAX_USER_TAGS then
                    return ctx.msg:reply("error: the max tags owned limit on " .. tags.MAX_USER_TAGS .. " tags has been reached"):await()
                end

                local error = tags.create_tag(ctx.msg.author, ctx.msg.channel.server, ctx.args.tag, value):await()

                if error then
                    return ctx.msg:reply("error: " .. ctx.msg.channel:escape_text(error)):await()
                else
                    return ctx.msg:reply("sucessfully created tag \"" .. ctx.msg.channel:escape_text(ctx.args.tag) .. "\""):await()
                end
            end,
        }),
        bot.sub_command("delete", {
            args = {
                {
                    key = "tag",
                    name = "NAME",
                    description = "Tag name",
                    required = true,
                },
                {
                    key = "force",
                    long = "force",
                    description = "Force (admin)",
                }
            },
            description = "Delete a tag",
            callback = function(ctx)
                local tag = tags.find_tag(ctx.msg.channel.server, ctx.args.tag):await()

                if not tag then
                    return ctx.msg:reply("error: unknown tag"):await()
                end

                if not ctx.args.force or not bot.has_role_or_higher("admin", ctx.msg.author.role) then
                    if tag.uid ~= ctx.msg.author.uid then
                        return ctx.msg:reply("error: access denied"):await()
                    end
                end

                tag:delete():await()

                return ctx.msg:reply("the tag \"" .. ctx.msg.channel:escape_text(ctx.args.tag) .. "\" has been deleted"):await()
            end,
        }),
        bot.sub_command("edit", {
            args = {
                {
                    key = "tag",
                    name = "NAME",
                    description = "Tag name",
                    required = true,
                },
                {
                    key = "value",
                    name = "VALUE",
                    description = "Tag value",
                },
                {
                    key = "force",
                    long = "force",
                    description = "Force (admin)",
                }
            },
            description = "Edit a tag",
            callback = function(ctx)
                local tag = tags.find_tag(ctx.msg.channel.server, ctx.args.tag):await()

                if not tag then
                    return ctx.msg:reply("error: unknown tag"):await()
                end

                if not ctx.args.force or not bot.has_role_or_higher("admin", ctx.msg.author.role) then
                    if tag.uid ~= ctx.msg.author.uid then
                        return ctx.msg:reply("error: access denied"):await()
                    end
                end

                local value = ctx.args.value or ""

                if #ctx.extra_args > 0 then
                    value = value .. " " .. table.concat(ctx.extra_args, " ")
                end

                for i, attachment in pairs(ctx.msg.attachments) do
                    if value ~= "" then value = value .. "\n" end

                    value = value .. attachment.url
                end

                if #value > tags.MAX_VALUE_LIMIT then
                    return ctx.msg:reply("error: the tag value cannot be longer than " .. tags.MAX_VALUE_LIMIT .. " characters"):await()
                end

                if #value == 0 then
                    return ctx.msg:reply("error: the tag value cannot be empty"):await()
                end

                tag:edit(value):await()

                return ctx.msg:reply("the tag \"" .. ctx.msg.channel:escape_text(ctx.args.tag) .. "\" has been edited"):await()
            end,
        }),
        bot.sub_command("list", {
            args = {
                {
                    key = "user",
                    name = "USER",
                    description = "User (optional)",
                },
                {
                    key = "page",
                    name = "PAGE",
                    description = "Page number",
                },
            },
            description = "List your own or someone else's tags",
            callback = function(ctx)
                local user

                if args.user then
                    user = bot.find_user(ctx.msg.channel, ctx.args.user):await()
                else
                    user = ctx.msg.author
                end

                if user then
                   local tag_names = tags.list_tags(user, ctx.msg.channel.server):await()

                   table.sort(tag_names, function(a, b) return a < b end)

                   return pagination.create(ctx.msg.channel, {
                        title = user.name .. "'s tags",
                        data = tag_names,
                        render_data = function(ctx, tag_names)
                            local content = ""

                            local i = ctx.offset
            
                            for _,name in pairs(tag_names) do
                                if content ~= "" then content = content .. "\n" end
            
                                content = content .. i .. ". \"" .. name .. "\""

                                i = i + 1
                            end
            
                            return {
                                content = content
                            }
                        end,
                        page = ctx.args.page,
                        caller = ctx.msg.author
                    })
                else
                    return ctx.msg:reply("error: no user found for \""..ctx.msg.channel:escape_text(ctx.args.user).."\""):await()
                end
            end,
        }),
        bot.sub_command("raw", {
            args = {
                {
                    key = "tag",
                    name = "NAME",
                    description = "Tag name",
                    required = true,
                },
            },
            description = "View the raw tag",
            callback = function(ctx)
                local tag = tags.find_tag(ctx.msg.channel.server, ctx.args.tag):await()

                if tag then
                    return ctx.msg:reply(ctx.msg.channel:escape_text(tag.value)):await()
                else
                    return ctx.msg:reply("error: unknown tag"):await()
                end
            end,
        }),
        bot.sub_command("owner", {
            args = {
                {
                    key = "tag",
                    name = "NAME",
                    description = "Tag name",
                    required = true,
                },
            },
            description = "Get the owner of a tag",
            callback = function(ctx)
                local tag = tags.find_tag(ctx.msg.channel.server, ctx.args.tag):await()

                if tag then
                    local owner = bot.get_user(tag.uid):await()

                    return ctx.msg:reply(ctx.msg.channel:escape_text(owner.name) .. " is the owner of the tag \"" .. ctx.msg.channel:escape_text(ctx.args.tag) .. "\""):await()
                else
                    return ctx.msg:reply("error: unknown tag"):await()
                end
            end,
        }),
        bot.sub_command("gift", {
            args = {
                {
                    key = "tag",
                    name = "NAME",
                    description = "Tag name",
                    required = true,
                },
                {
                    key = "user",
                    name = "USER",
                    description = "User (empty to abort transfer)",
                    required = false,
                },
            },
            description = "Gift a tag to another user",
            callback = function(ctx)
                local tag = tags.find_tag(ctx.msg.channel.server, ctx.args.tag):await()

                if not tag then
                    return ctx.msg:reply("error: unknown tag"):await()
                end

                if tag.uid ~= ctx.msg.author.uid then
                    return ctx.msg:reply("error: access denied"):await()
                end

                if ctx.args.user then
                    local user = bot.find_user(ctx.msg.channel, ctx.args.user):await()

                    if user.uid == ctx.msg.author.uid then
                        return ctx.msg:reply("error: you cannot transfer to yourself"):await()
                    end

                    if user then
                        tag:set_transfer_user(user):await()
                        return ctx.msg:reply(ctx.msg.channel:escape_text(user.name) .. " can now do \"tag accept "..ctx.msg.channel:escape_text(ctx.args.tag).."\" to accept the tag transfer"):await()
                    else
                        return ctx.msg:reply("error: no user found for \""..ctx.msg.channel:escape_text(ctx.args.user).."\""):await()
                    end
                else
                    tag:set_transfer_user(nil):await()
                    return ctx.msg:reply("removed transfer state from \""..ctx.msg.channel:escape_text(ctx.args.tag).."\""):await()
                end
            end,
        }),
        bot.sub_command("accept", {
            args = {
                {
                    key = "tag",
                    name = "NAME",
                    description = "Tag name",
                    required = true,
                },
                {
                    key = "force",
                    long = "force",
                    description = "Force (admin)",
                }
            },
            description = "Accept a gifted tag",
            callback = function(ctx)
                local tag = tags.find_tag(ctx.msg.channel.server, ctx.args.tag):await()

                if not tag then
                    return ctx.msg:reply("error: unknown tag")
                end

                if not ctx.args.force or not bot.has_role_or_higher("admin", ctx.msg.author.role) then
                    if tag.transfer_uid ~= ctx.msg.author.uid then
                        return ctx.msg:reply("error: the tag is not being transfered to you"):await()
                    end
                end

                tag:set_transfer_user(nil):await()
                tag:set_owner(ctx.msg.author):await()

                return ctx.msg:reply("the tag \""..ctx.msg.channel:escape_text(ctx.args.tag).."\" is now yours"):await()
            end,
        }),
    }
})
