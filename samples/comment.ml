(* Retry only if the remote service
   reports a temporary failure. *)
let retry_count = 3

(** The delay before the next retry, in milliseconds.
    It doubles with each attempt. *)
let retry_delay attempt = 200 * (1 lsl attempt)

(* A comment can hold "*)" in a string (* and nest *). *)
let separator = "(* not a comment *)"
let quoted = {|(* not a comment either *)|}
