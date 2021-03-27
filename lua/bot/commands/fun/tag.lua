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
    callback = function(msg, args, extra_args)
        local tag = tags.find_tag(msg.channel.server, args.tag):await()

        if tag then
            return msg:reply(msg.channel:escape_text(tags.exec_tag(msg, msg.author, msg.channel, tag, extra_args))):await()
        else
            return msg:reply("error: unknown tag"):await()
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
            callback = function(msg, args, extra_args)
                if not tags.is_valid_name(args.tag) then
                    return msg:reply("error: the tag name must be alphanumeric"):await()
                end

                if #args.tag > tags.MAX_NAME_LIMIT then
                    return msg:reply("error: the tag name cannot be longer than " .. tags.MAX_NAME_LIMIT .. " characters"):await()
                end

                local value = args.value or ""

                if #extra_args > 0 then
                    value = value .. " " .. table.concat(extra_args, " ")
                end

                for i, attachment in pairs(msg.attachments) do
                    if value ~= "" then value = value .. "\n" end

                    value = value .. attachment.url
                end

                if #value > tags.MAX_VALUE_LIMIT then
                    return msg:reply("error: the tag value cannot be longer than " .. tags.MAX_VALUE_LIMIT .. " characters"):await()
                end

                if #value == 0 then
                    return msg:reply("error: the tag value cannot be empty"):await()
                end

                if tags.count_user_tags(msg.author):await() > tags.MAX_USER_TAGS then
                    return msg:reply("error: the max tags owned limit on " .. tags.MAX_USER_TAGS .. " tags has been reached"):await()
                end

                local error = tags.create_tag(msg.author, msg.channel.server, args.tag, value):await()

                if error then
                    return msg:reply("error: " .. msg.channel:escape_text(error)):await()
                else
                    return msg:reply("sucessfully created tag \"" .. msg.channel:escape_text(args.tag) .. "\""):await()
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
            callback = function(msg, args)
                local tag = tags.find_tag(msg.channel.server, args.tag):await()

                if not tag then
                    return msg:reply("error: unknown tag"):await()
                end

                if not args.force or not bot.has_role_or_higher("admin", msg.author.role) then
                    if tag.uid ~= msg.author.uid then
                        return msg:reply("error: access denied"):await()
                    end
                end

                tag:delete():await()

                return msg:reply("the tag \"" .. msg.channel:escape_text(args.tag) .. "\" has been deleted"):await()
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
            callback = function(msg, args, extra_args)
                local tag = tags.find_tag(msg.channel.server, args.tag):await()

                if not tag then
                    return msg:reply("error: unknown tag"):await()
                end

                if not args.force or not bot.has_role_or_higher("admin", msg.author.role) then
                    if tag.uid ~= msg.author.uid then
                        return msg:reply("error: access denied"):await()
                    end
                end

                local value = args.value or ""

                if #extra_args > 0 then
                    value = value .. " " .. table.concat(extra_args, " ")
                end

                for i, attachment in pairs(msg.attachments) do
                    if value ~= "" then value = value .. "\n" end

                    value = value .. attachment.url
                end

                if #value > tags.MAX_VALUE_LIMIT then
                    return msg:reply("error: the tag value cannot be longer than " .. tags.MAX_VALUE_LIMIT .. " characters"):await()
                end

                if #value == 0 then
                    return msg:reply("error: the tag value cannot be empty"):await()
                end

                tag:edit(value):await()

                return msg:reply("the tag \"" .. msg.channel:escape_text(args.tag) .. "\" has been edited"):await()
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
            callback = function(msg, args)
                local user

                if args.user then
                    user = bot.find_user(msg.channel, args.user):await()
                else
                    user = msg.author
                end

                if user then
                   local tag_names = tags.list_tags(user, msg.channel.server):await()

                   table.sort(tag_names, function(a, b) return a < b end)

                   return pagination.create(msg.channel, {
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
                        page = args.page,
                        caller = msg.author
                    })
                else
                    return msg:reply("error: no user found for \""..msg.channel:escape_text(args.user).."\""):await()
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
            callback = function(msg, args)
                local tag = tags.find_tag(msg.channel.server, args.tag):await()

                if tag then
                    return msg:reply(msg.channel:escape_text(tag.value)):await()
                else
                    return msg:reply("error: unknown tag"):await()
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
            callback = function(msg, args)
                local tag = tags.find_tag(msg.channel.server, args.tag):await()

                if tag then
                    local owner = bot.get_user(tag.uid):await()

                    return msg:reply(msg.channel:escape_text(owner.name) .. " is the owner of the tag \"" .. msg.channel:escape_text(args.tag) .. "\""):await()
                else
                    return msg:reply("error: unknown tag"):await()
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
            callback = function(msg, args)
                local tag = tags.find_tag(msg.channel.server, args.tag):await()

                if not tag then
                    return msg:reply("error: unknown tag"):await()
                end

                if tag.uid ~= msg.author.uid then
                    return msg:reply("error: access denied"):await()
                end

                if args.user then
                    local user = bot.find_user(msg.channel, args.user):await()

                    if user.uid == msg.author.uid then
                        return msg:reply("error: you cannot transfer to yourself"):await()
                    end

                    if user then
                        tag:set_transfer_user(user):await()
                        return msg:reply(msg.channel:escape_text(user.name) .. " can now do \"tag accept "..msg.channel:escape_text(args.tag).."\" to accept the tag transfer"):await()
                    else
                        return msg:reply("error: no user found for \""..msg.channel:escape_text(args.user).."\""):await()
                    end
                else
                    tag:set_transfer_user(nil):await()
                    return msg:reply("removed transfer state from \""..msg.channel:escape_text(args.tag).."\""):await()
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
            callback = function(msg, args)
                local tag = tags.find_tag(msg.channel.server, args.tag):await()

                if not tag then
                    return msg:reply("error: unknown tag")
                end

                if not args.force or not bot.has_role_or_higher("admin", msg.author.role) then
                    if tag.transfer_uid ~= msg.author.uid then
                        return msg:reply("error: the tag is not being transfered to you"):await()
                    end
                end

                tag:set_transfer_user(nil):await()
                tag:set_owner(msg.author):await()

                return msg:reply("the tag \""..msg.channel:escape_text(args.tag).."\" is now yours"):await()
            end,
        }),
    }
})