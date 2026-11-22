# Retry only if the remote service
# reports a temporary failure.
set(retry_count 3) # Use a small retry budget.

#[[
  Delay the next request after a failure.
  Leave enough time for the service to recover.
]]
#[==[ The shorter close ]] can appear in this comment. ]==]
set(label "# literal")
set(description [[# bracket text]])
set(marker \#escaped)
