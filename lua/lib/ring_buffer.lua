local ring_buffer = {}

function ring_buffer:push(value)
    self.offset = (self.offset % self.capacity) + 1
    self.tbl[self.offset] = value
end

function ring_buffer:get(offset)
    local offset = ((self.offset + offset - 1) % self:get_size()) + 1

    return self.tbl[offset]
end

function ring_buffer:get_capacity()
    return self.capacity
end


function ring_buffer:get_size()
    return #self.tbl
end

return function(capacity)
    local buffer = {}

    buffer.tbl = {}
    buffer.capacity = capacity
    buffer.offset = 0

    setmetatable(buffer, { __index = ring_buffer })
    return buffer
end
