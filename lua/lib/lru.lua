local lru = {}

local KEY = 1
local VALUE = 2
local NEXT = 3
local PREV = 4

function lru:_remove(entry)
    local next = entry[NEXT]
    local prev = entry[PREV]
    entry[NEXT] = nil
    entry[PREV] = nil

    if next and prev then
        next[PREV] = prev
        prev[NEXT] = next
    elseif next then
        next[PREV] = nil
        self.newest = next
    elseif prev then
        prev[NEXT] = nil
        self.oldest = prev
    else
        self.newest = nil
        self.oldest = nil
    end
end

function lru:_set_newest(entry)
    if not self.newest then
        self.newest = entry
        self.oldest = entry
    else
        entry[NEXT] = self.newest
        self.newest[PREV] = entry
        self.newest = entry
    end
end

function lru:set(key, value)
    assert(key ~= nil)

    local entry = self.entries[key]
    if entry then
        self:_remove(entry)
    end

    if value ~= nil then
        if self.oldest and self:get_size() >= self:get_capacity() then
            self.entries[self.oldest[KEY]] = nil
            self:_remove(self.oldest)
        end

        local new_entry = entry or {key, nil, nil, nil}
        new_entry[VALUE] = value
        self:_set_newest(new_entry)
        self.entries[key] = new_entry
    end
end

function lru:delete(key)
    self:set(key, nil)
end

function lru:get(key)
    local entry = self.entries[key]
    if not entry then return end

    self:_remove(entry)
    self:_set_newest(entry)
    return entry[VALUE]
end

function lru:get_capacity()
    return self.capacity
end


function lru:get_size()
    return #self.entries
end

return function(capacity)
    local cache = {}

    cache.newest = nil
    cache.oldest = nil
    cache.entries = {}
    cache.capacity = capacity

    setmetatable(cache, { __index = lru })
    return cache
end
