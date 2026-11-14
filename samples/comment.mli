(* Retry only if the remote service
   reports a temporary failure. *)
val retry_count : int

(** The delay before the next retry, in milliseconds. *)
val retry_delay : int -> int
