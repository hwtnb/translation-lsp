// Retry only if the remote service
// reports a temporary failure.
val retry_count = 3

/** The delay before the next retry, in milliseconds.
  * It doubles with each attempt.
  */
def retryDelay(attempt: Int): Int = 200 * (1 << attempt)

/* A comment can hold /* a nested comment */ and go on. */
def temporary(status: Int): Boolean = status == 503 // 503 means "try again later".

val separator = "// not a comment"
val quoted = """/* not a comment either */"""
val message = s"// not a comment: ${retryDelay(1) /* The delay of the first retry. */}"
