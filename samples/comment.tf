# Retry only if the remote service
# reports a temporary failure.
locals {
  retry_count = 3

  // Keep both markers in the message.
  markers = "# and // are part of this string."

  /*
   * Show the configured retry count
   * in the status message.
   */
  status = "Retry up to ${local.retry_count} times."

  script = <<-SCRIPT
    # This line is part of the shell script.
    echo "Retry"
  SCRIPT
  # The shell script ends above this line.
}
