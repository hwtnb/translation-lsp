// Retry only if the remote service
// reports a temporary failure.
const retry_count: number = 3;

type Props = { attempt: number };

export function Status({ attempt }: Props) {
  return (
    <p data-retry_count={retry_count}>
      {/* Shown while the next attempt waits. */}
      Attempt {attempt}. See https://example.com/status for details.
    </p>
  );
}
