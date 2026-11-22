-- Retry only if the remote service
-- reports a temporary failure.
def retry_count := 3 -- Use a small retry budget.

/--
  Compute the next backoff delay in milliseconds.
  Double the base interval on each attempt.
-/
def retryDelay (attempt : Nat) := 200 * (2 ^ attempt)

/-! The manual describes the retry policy. -/

/-
  Keep the retry count below the budget. /- This note is nested. -/
    Count the first attempt in the budget.
-/
def «retry -- count» := retry_count
def literal := "-- literal"
def raw := r"/- raw -/"
def quote := '"'
def summary := s!"status -- in string {retry_count /- Format the attempt count. -/}"
