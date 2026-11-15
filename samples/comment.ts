// Retry only if the remote service
// reports a temporary failure.
const retry_count: number = 3;

/**
 * Returns the wait before the given attempt.
 *
 * @param attempt - The number of attempts so far.
 * @returns The wait in milliseconds.
 */
export function delay(attempt: number): number {
  return Math.min(100 * 2 ** attempt, 60000); // Never longer than a minute.
}

interface Policy {
  limit: number;
  codes: readonly number[];
}

export const policy: Policy = { limit: retry_count, codes: [429, 503] };
