async = async or {}

local async = async

async.FUTURE_STATE = {
    Pending = 1,
    Executing = 2,
    Resolved = 3,
    Rejected = 4
}

async.CALLBACK_TYPE = {
    Then = 1,
    Catch = 2,
}

async.FutureMeta = async.FutureMeta or {}
async.FutureMeta.__index = async.FutureMeta

local next_tick_cbs = {}

function async.FutureMeta:thence(callback)
    if self.state == async.FUTURE_STATE.Pending then
        table.insert(self.callbacks, { type = async.CALLBACK_TYPE.Then, cb = callback })
    elseif self.state == async.FUTURE_STATE.Resolved then
        callback(table.unpack(self.results))
    end

    return self
end

function async.FutureMeta:catch(callback)
    if self.state == async.FUTURE_STATE.Pending then
        table.insert(self.callbacks, { type = async.CALLBACK_TYPE.Catch, cb = callback })
    elseif self.state == async.FUTURE_STATE.Rejected then
        callback(table.unpack(self.results))
    end

    return self
end

-- Works only inside of a coroutine pool
function async.FutureMeta:await()
    while true do
        coroutine.yield()

        local res = self.results

        if self.state ~= async.FUTURE_STATE.Pending and self.state ~= async.FUTURE_STATE.Executing then
            if self.state == async.FUTURE_STATE.Resolved then
                return table.unpack(res, 1, #res)
            else
                error(res[1])
            end
        end
    end
end

function async.FutureMeta:__handle_resolve(force, ...)
    if not force and self.state ~= async.FUTURE_STATE.Executing then return end
    self.state = async.FUTURE_STATE.Resolved

    self.results = {...}

    local next_cb = table.remove(self.callbacks, 1)
    local last_res = self.results

    while next_cb do
        if next_cb.type == async.CALLBACK_TYPE.Then then
            local res = {pcall(next_cb.cb, table.unpack(last_res))}
            local succ = table.remove(res, 1)

            if succ then
                -- Resolve the future if it was the first result
                if res[1] and res[1].__type and res[1]:__type() == "future" then
                    local cur_fut = self

                    res[1]:thence(function(...)
                        cur_fut:__handle_resolve(true, ...)
                    end):catch(function(...)
                        cur_fut:__handle_reject(true, ...)
                    end)()

                    return
                else
                    last_res = res
                end
            else
                self:__handle_reject(true, table.unpack(res))
                return
            end
        end

        next_cb = table.remove(self.callbacks, 1)
    end
end

function async.FutureMeta:__handle_reject(force, ...)
    if not force and self.state ~= async.FUTURE_STATE.Executing then return end
    self.state = async.FUTURE_STATE.Rejected

    self.results = {...}
    local next_cb = table.remove(self.callbacks, 1)
    local last_res = self.results

    while next_cb do
        if next_cb.type == async.CALLBACK_TYPE.Catch then
            local res = {pcall(next_cb.cb, table.unpack(last_res))}
            local succ = table.remove(res, 1)

            if succ then
                return
            else
                last_res = res
            end
        end

        next_cb = table.remove(self.callbacks, 1)
    end
end

function async.FutureMeta:__call()
    if not self.future_fn then return end -- Engine futures
    if self.state ~= async.FUTURE_STATE.Pending then return self end
    self.state = async.FUTURE_STATE.Executing

    local succ, err = pcall(
        self.future_fn,
        function(...)
            return self:__handle_resolve(false, ...)
        end,
        function(...)
            return self:__handle_reject(false, ...)
        end
    )

    if not succ then
        self:__handle_reject(false, err)
    end

    return self
end

function async.FutureMeta:__tostring()
    return "Future"
end

function async.FutureMeta:__type()
    return "future"
end

function async.__RustFuture()
    local future = setmetatable({
        state = async.FUTURE_STATE.Pending,
        callbacks = {}
    }, async.FutureMeta)

    return future
end

function async.future(future_fn)
    assert(type(future_fn) == "function")

    local future = setmetatable({
        future_fn = future_fn,
        state = async.FUTURE_STATE.Pending,
        callbacks = {}
    }, async.FutureMeta)

    table.insert(next_tick_cbs, future)

    return future
end

function async.poll()
    for k,v in pairs(next_tick_cbs) do
        v()
        next_tick_cbs[k] = nil
    end
end
