function string.trim_lines(lines)
    local out = ""

    for line in lines:gmatch("[^\r\n]+") do
        if out ~= "" then
            out = out .. "\n"
        end

        out = out .. line:gsub("^%s*(.-)%s*$", "%1")
    end

    return out
end
