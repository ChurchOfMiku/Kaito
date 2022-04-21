tags = tags or {}

tags.MAX_NAME_LIMIT = 20
tags.MAX_VALUE_LIMIT = 2000
tags.MAX_USER_TAGS = 200

tags.VARS = {
    args = function(ctx) return table.concat(ctx.extra_args, "") end,
    argslen = function(ctx) return tostring(#ctx.extra_args) end,
    id = function(ctx) return ctx.user.id end,
    name = function(ctx) return ctx.user.name end
}

tags.SCRIPTED_TAGS = {
    arg = {
        fn = function(ctx, value)
            local num = tonumber(value)
            if num == nil then error("expected number as argument for arg script tag") end
            return tostring(ctx.extra_args[num])
        end
    },
    code = {
        fn = function(ctx, code)
            return bot.code_block(ctx.channel, code)
        end
    },
    choose = {
        fn = function(ctx, value)
            local picks = {}

            for pick in string.gmatch(value, "([^|]+)|?") do table.insert(picks, pick) end

            if #picks == 0 then return "" end
            return picks[math.random(1, #picks)]
        end
    },
    lua = {
        fn = function(ctx, code)
            local env = {
                args = ctx.extra_args,
                user = {
                    name = ctx.user.name,
                    id = ctx.user.id
                }
            }

            local err, res = bot.run_sandboxed_lua(ctx.user, ctx.msg, code, env, ctx.tag):await()
            if err then
                error(err)
            end

            return res
        end
    },
    lower = {
        fn = function(ctx, value)
            return string.lower(value)
        end
    },
    note = {
        fn = function(ctx, _)
            return ""
        end
    },
    upper = {
        fn = function(ctx, value)
            return string.upper(value)
        end
    },
    range = {
        fn = function(ctx, value)
            local sep = string.find(value, "|")
            if not sep then error("expected | seperator inside of range script tag") end
            local a, b = tonumber(string.sub(value, 0, sep - 1)), tonumber(string.sub(value, sep + 1))
            if a == nil or b == nil then error("expected two numbers seperated by | inside of range script tag") end
            return tostring(math.random(a, b))
        end
    }
}

function tags.is_valid_name(name)
    return string.match(name, "[^%w_]") == nil
end

function tags.exec_tag(msg, user, channel, tag, extra_args)
    local out = ""
    local ctx = {
        msg = msg,
        user = user,
        channel = channel,
        tag = tag,
        extra_args = extra_args
    }

    for k,tag_part in ipairs(tags.parse_tag(tag.value)) do
        if type(tag_part) == "string" then
            out = out .. tag_part
        else
            if tag_part.var then
                if tags.VARS[tag_part.var] then
                    out = out .. tostring(tags.VARS[tag_part.var](ctx) or "")
                else
                    out = out .. "{" .. tag_part.var .. "}"
                end
            elseif tag_part.tag then
                if tags.SCRIPTED_TAGS[tag_part.tag] then
                    out = out .. tags.SCRIPTED_TAGS[tag_part.tag].fn(ctx, tag_part.value)
                else
                    out = out .. "{" .. tag_part.tag .. ":" .. tag_part.value .. "}"
                end
            elseif tag_part.codeblock then
                if tags.SCRIPTED_TAGS[tag_part.codeblock] then
                    out = out .. tags.SCRIPTED_TAGS[tag_part.codeblock].fn(ctx, tag_part.value)
                else
                    out = out .. "```\n" .. tag_part.codeblock .. " " .. tag_part.value .. "\n```"
                end
            end
        end
    end

    return out
end
