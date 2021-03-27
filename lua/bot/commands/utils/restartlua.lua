bot.add_command("restartlua", {
    description = "Restarts the sandbox lua state",
    callback = function(ctx)
        bot.restart_sandbox()
    end,
    role = "trusted",
})
