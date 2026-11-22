// Retry only if the remote service
// reports a temporary failure.
let retry_count = 3 // Use a small retry budget.

/// Compute the next backoff delay in milliseconds.
/// Double the base interval on each attempt.
let retryDelay attempt = (*) 200 (1 <<< attempt)

(*
  Keep a literal "*)" inside a string (* and a nested block comment *)
  so the outer comment ends at the matching delimiter.
*)
let ``path // name`` = "// literal"
let verbatim = @"(* verbatim *)"
let quote = '"'
let summary = $"status // in string {(* Format the attempt count. *) retry_count}"
