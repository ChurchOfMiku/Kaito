bot = bot or {}
bot.cmds = bot.cmds or {}
bot.aliases = bot.aliases or {}
bot.reaction_hooks = {}

include("./lib/async.lua")
include("./lib/hooks.lua")
include("./lib/pagination.lua")
include("./lib/string.lua")
include("./lib/tags.lua")
RingBuffer = include("./lib/ring_buffer.lua")

function bot.think()
end

local function get_abs_cmd(cmd)
    if cmd._parent_cmd then
        local t = cmd.cmd
        local cmd = cmd._parent_cmd

        while cmd do
            t = cmd.cmd .. " " .. t
            cmd = cmd._parent_cmd
        end

        return t
    else
        return cmd.cmd
    end
end

local function create_command(cmd, options)
    options.cmd = cmd
    options.sub_commands = options.sub_commands or {}
    options._sub_commands = options._sub_commands or {}

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

    for _,v in pairs(options.sub_commands) do
        v._parent_cmd = options
        options._sub_commands[v.cmd] = v
    end

    return options
end

function bot.add_command(cmd_name, options)
    local cmd = create_command(cmd_name, options)
    bot.cmds[cmd_name] = cmd

    if options.aliases then
        for k,v in pairs(options.aliases) do
            bot.aliases[v] = cmd
        end
    end
end

function bot.sub_command(cmd, options)
    return create_command(cmd, options)
end

function bot.icode_block(channel, content)
    local a = channel:supports_feature(bot.FEATURES.Markdown) and "``" or ""
    return a .. content .. a
end

function bot.code_block(channel, content)
    local a = channel:supports_feature(bot.FEATURES.Markdown) and "```\n" or ""
    local b = channel:supports_feature(bot.FEATURES.Markdown) and "\n```" or ""
    return a .. content .. b
end

function bot.bold_block(channel, content)
    local a = channel:supports_feature(bot.FEATURES.Markdown) and "**" or ""
    return a .. content .. a
end

function bot.help(msg, cmd)
    local usage_options = ""
    local usage_arguments = ""

    local arguments = ""
    local options = ""
    local sub_commands = ""

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
                "   " .. bot.icode_block(msg.channel,
                    ((v.short and "-" or "") .. (v.short or "") .. (v.short and ",  " or "")) ..
                        (long .. string.rep(" ", pad - #long)) .. v.description) .. "\n"
        end
    end

    if #(cmd._arguments) > 0 then
        arguments = "\n\nARGUMENTS:\n"

        local min_len = 0

        local len = #cmd._arguments
        for k, v in ipairs(cmd._arguments) do
            local name = v.name or v.key
            usage_arguments = usage_arguments .. "[" .. name .. "]" .. (k ~= len and " " or "")
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
            else
                extra = ""
            end

            arguments =
                arguments .. "   " .. bot.icode_block(msg.channel, name .. string.rep(" ", pad - #name) .. (v.description or "") .. extra) .. "\n"
        end
    end

    if #(cmd.sub_commands) > 0 then
        if usage_options ~= "" then
            usage_options = usage_options .. " "
        end

        usage_options = usage_options .. "<SUBCOMMAND> "

        sub_commands = "\nSUBCOMMANDS:\n"

        local min_len = 0

        for _, v in ipairs(cmd.sub_commands) do
            min_len = math.max(min_len, #v.cmd)
        end

        local pad = min_len + 3

        for _, v in ipairs(cmd.sub_commands) do
            sub_commands =
                sub_commands .. "   " .. bot.icode_block(msg.channel, v.cmd .. string.rep(" ", pad - #v.cmd) .. (v.description or "")) .. "\n"
        end
    end

    local out =
        get_abs_cmd(cmd) ..
        "\n" ..
            ((cmd.description and cmd.description .. "\n") or "") ..
                "\n" .. "USAGE:\n   " .. bot.icode_block(msg.channel, get_abs_cmd(cmd) .. " " .. usage_options .. usage_arguments) .. arguments .. options .. sub_commands

    msg:reply(out)
end

function bot.parse_args(cmd, args)
    local out = {}
    local extra_args = {}

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
            else
                out[opt.key] = true
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

                if argument then
                    out[argument.key] = arg
                    arg_index = arg_index + 1
                else
                    table.insert(extra_args, arg)
                end
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

    return true, out, extra_args
end

function bot.has_role_or_higher(role, user_role, only_higher)
    local function entry_index(tbl, val)
        for k,v in ipairs(tbl) do
            if v == val then
                return k
            end
        end

        error("unknown role "..val)
    end

    if not only_higher and role == user_role then return true end

    local role_idx = entry_index(bot.ROLES, role)
    local user_role_idx = entry_index(bot.ROLES, user_role)

    return user_role_idx > role_idx
end

local function exec_command(msg, cmd, args)
    local has_subcommands = #cmd.sub_commands > 0

    if not cmd then
        return
    end

    if cmd.role then
        if not bot.has_role_or_higher(cmd.role, msg.author.role) then
            return msg:reply("permission denied: this command requires the role of  " .. cmd.role .. " or higher.")
        end
    end

    if has_subcommands then
        local cmd_name = args[1]
        local args = {table.unpack(args, 2, #args)}

        if cmd_name then
            local sub_cmd = cmd._sub_commands[cmd_name]

            if sub_cmd then
                return exec_command(msg, sub_cmd, args)
            end
        end

        if not cmd.callback then
            return bot.help(msg, cmd)
        end
    end

    if bot.utils.array_has_value(args, "--help") then
        return bot.help(msg, cmd)
    end

    local succ, res, extra_args = bot.parse_args(cmd, args)

    if not succ then
        return msg:reply("argument error: " .. res .. '\nUse "' .. get_abs_cmd(cmd) .. ' --help" for more info.')
    end

    cmd.callback(msg, res, extra_args)
end

function bot.on_command(msg, args)
    local cmd_name = args[1]
    local args = {table.unpack(args, 2, #args)}

    local cmd = bot.cmds[cmd_name] or bot.aliases[cmd_name]

    if not cmd then
        return
    end

    exec_command(msg, cmd, args)
end

function bot.on_message(msg)
    hooks.call("message", msg)
end

function bot.on_reaction(msg, reactor, reaction, removed)
    if bot.reaction_hooks[msg.id] then
        bot.reaction_hooks[msg.id](msg, reactor, reaction, removed)
    end
end

function bot.shutdown()
    hooks.call("shutdown")
end

include("bot/utils.lua")
include("bot/commands/**/*.lua")
include("bot/modules/**/*.lua")
