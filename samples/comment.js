// Retry only if the remote service
// reports a temporary failure.
const retry_count = 3;

/**
 * Returns the wait before the given attempt.
 * The wait doubles with each attempt.
 * @param {number} attempt The number of attempts so far.
 * @returns {number} The wait in milliseconds.
 */
function delay(attempt) {
  return Math.min(100 * 2 ** attempt, 60000); // Never longer than a minute.
}

const status = "https://example.com/status // not a comment";
const separators = /[/*]+/g;
const message = `Attempt ${retry_count} of ${/* The largest number of attempts. */ 5} // not a comment either`;

export { delay, message, separators, status };
