-- Retry only if the remote service
-- reports a temporary failure.
retry_count :: Int
retry_count = 3

-- | The delay before the next retry, in milliseconds.
-- It doubles with each attempt.
retryDelay :: Int -> Int
retryDelay attempt = 200 * 2 ^ attempt

{- The service reports a temporary failure
   with status 503. -}
temporary :: Int -> Bool
temporary status = status == 503 -- 503 means "try again later".

separator :: String
separator = "-- not a comment"

(-->) :: Int -> Int -> Int
a --> b = a + b
