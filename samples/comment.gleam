//// Retry only if the remote service
//// reports a temporary failure.

/// Builds the label for a retry attempt.
/// The label includes the name of the operation.
pub fn retry_label() -> String {
  // Keep the protocol separator in the URL.
  let url = "https://example.com/retry"
  let retry_count = 3 // Use the default number of attempts.
  let operation = "Run \"retry\" //retry" // Include the quoted command.
  operation <> " at " <> url <> " (" <> int_label(retry_count) <> ")"
}

fn int_label(value: Int) -> String {
  case value {
    3 -> "three attempts"
    _ -> "custom number of attempts"
  }
}
