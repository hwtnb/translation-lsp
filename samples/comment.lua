-- Retry only if the remote service
-- reports a temporary failure.
local retry_count = 3

--[[
  Wait before the next attempt, as in this example.

      wait(retry_count)
]]
