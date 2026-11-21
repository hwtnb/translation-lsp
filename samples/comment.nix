# Retry only if the remote service
# reports a temporary failure.
let
  retry_count = 3;

  /*
   * Keep the markers in the status message
   * so they can be shown as written.
   */
  status = "Retry up to ${toString retry_count} times. # status";

  /** The shell script announces each retry. */
  script = ''
    # This line is part of the shell script.
    echo "Retry"
  '';
in
{ inherit script status; } // { url = "https://example.com/#retry"; }
