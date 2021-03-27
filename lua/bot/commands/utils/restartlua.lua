bot.add_command("restartlua", {
    description = "Restarts the sandbox lua state",
    callback = function(msg, args)
        bot.restart_sandbox()
    end,
    role = "trusted",
})
