// Retry only if the remote service
// reports a temporary failure.
const retry_count = 3;

export function Status() {
  return (
    <p data-retry_count={retry_count}>
      {/* Shown while the next attempt waits. */}
      See https://example.com/status for details.
    </p>
  );
}
