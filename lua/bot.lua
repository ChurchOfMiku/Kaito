bot = bot or {}
bot.cmds = bot.cmds or {}

bot.think = function()
end

bot.add_command = function(cmd, options)
    options.cmd = cmd
    -- Needed for parsing and help
    options._arguments = {}
    options._options = {}
    options._long_options = {}
    options._short_options = {}

    for _, v in ipairs(options.args or {}) do
        -- Check if it is an option
        if v.long or v.short then
            table.insert(options._options, v)

            if v.long then
                options._long_options[v.long] = v
            end

            if v.short then
                options._short_options[v.short] = v
            end
        else
            table.insert(options._arguments, v)
        end
    end

    bot.cmds[cmd] = options
end

bot.help = function(msg, cmd)
    local usage_options = ""
    local usage_arguments = ""

    local arguments = ""
    local options = ""

    if #(cmd._options) > 0 then
        usage_options = "[OPTIONS] "

        options = "\nOPTIONS:\n"

        local min_len = 0

        for _, v in ipairs(cmd._options) do
            local long = v.long and "--" .. v.long or ""
            min_len = math.max(min_len, #long)
        end

        local pad = min_len + 3

        for _, v in ipairs(cmd._options) do
            local long = v.long and "--" .. v.long or ""
            options =
                options ..
                "   " ..
                    (v.short and "-" .. v.short .. (v.long and ",  " or "   ")) ..
                        (long .. string.rep(" ", pad - #long)) .. v.description
        end
    end

    if #(cmd._arguments) > 0 then
        arguments = "\n\nARGUMENTS:\n"

        local min_len = 0

        for _, v in ipairs(cmd._arguments) do
            local name = v.name or v.key
            usage_arguments = usage_arguments .. "<" .. name .. "> "
            min_len = math.max(min_len, #name)
        end

        local pad = min_len + 3

        for _, v in ipairs(cmd._arguments) do
            local name = v.name or v.key
            local extra = {}

            if v.required then
                table.insert(extra, "required")
            end

            if #extra > 0 then
                extra = " (" .. table.concat(extra, ", ") .. ")"
            end

            arguments =
                arguments .. "   " .. name .. string.rep(" ", pad - #name) .. (v.description or "") .. extra .. "\n"
        end
    end

    local out =
        cmd.cmd ..
        "\n" ..
            ((cmd.description and cmd.description .. "\n") or "") ..
                "\n" .. "USAGE:\n   " .. cmd.cmd .. " " .. usage_options .. usage_arguments .. arguments .. options

    msg:reply(out)
end

bot.parse_args = function(cmd, args)
    local out = {}

    local taking_opt_value
    local arg_index = 1

    local function starts_with(str, start)
        return str:sub(1, #start) == start
    end

    for _, arg in ipairs(args) do
        if starts_with(arg, "--") then
            local opt_name = string.sub(arg, 3)
            local opt = cmd._long_options[opt_name]

            if taking_opt_value then
                return false, 'expected value for "--' .. taking_opt_value .. '", not "--' .. opt_name .. '"'
            end

            if not opt then
                return false, 'unknown option "--' .. opt_name .. '"'
            end

            if opt.takes_value then
                taking_opt_value = opt_name
            end
        elseif starts_with(arg, "-") then
            local opt_name = string.sub(arg, 2, 2)
            local opt = cmd._short_options[opt_name]

            if taking_opt_value then
                return false, 'expected value for "--' .. taking_opt_value .. '", not "-' .. opt_name '"'
            end

            if not opt then
                return false, 'unknown option "-' .. opt_name .. '"'
            end

            local value = string.sub(arg, 2)

            out[arg.key] = value ~= "" and value or true
        else
            if taking_opt_value then
                local opt = cmd._long_options[taking_opt_value]
                out[arg.key] = arg
                taking_opt_value = nil
            else
                local argument = cmd._arguments[arg_index]

                if not argument then
                    return false, "too many arguments"
                end

                out[argument.key] = arg

                arg_index = arg_index + 1
            end
        end
    end

    for _, arg in ipairs(cmd._arguments) do
        if arg.required then
            if not out[arg.key] then
                return false, 'missing argument "' .. (arg.name or arg.key) .. '"'
            end
        end
    end

    return true, out
end

bot.on_command = function(msg, args)
    local cmd_name = args[1]
    local args = {table.unpack(args, 2, #args)}

    local cmd = bot.cmds[cmd_name]

    if not cmd then
        return
    end

    if bot.utils.array_has_value(args, "--help") then
        return bot.help(msg, cmd)
    end

    local succ, res = bot.parse_args(cmd, args)

    if not succ then
        return msg:reply("argument error: " .. res .. '\nUse "' .. cmd_name .. ' --help" for more info.')
    end

    cmd.callback(msg, res)
end

include("bot/utils.lua")
include("bot/commands/**/*.lua")
