# Retry policy

This document describes when a client sends a failed request again and how long it waits first. The rules apply to every request that changes no data on the server.

## When to retry

Retry a request only when the failure is *temporary*. A temporary failure is one that the same request can avoid a few seconds later, such as a busy server or a dropped connection. Do not retry a request that the server rejected because the request itself was wrong.

The following count as temporary failures:

- A response with status `429 Too Many Requests`
- A response with status `503 Service Unavailable`
- A connection that closed before any response arrived

## How long to wait

Wait longer after each failed attempt, so that the server gets fewer requests while it recovers.

1. Wait one second before the first retry.
2. Double the wait before each further retry.
3. Stop after the number of attempts that `retry_count` sets.

When the response has a `Retry-After` header, wait **at least** as long as it asks, even when the rules above give a shorter wait.

| Attempt | Wait before it | Total wait so far |
| --- | --- | --- |
| 1 | None | 0 seconds |
| 2 | 1 second | 1 second |
| 3 | 2 seconds | 3 seconds |
| 4 | 4 seconds | 7 seconds |

## Example

The function below returns the wait before a given attempt. It never waits longer than one minute.

```js
function delay(attempt) {
  // The first attempt does not wait.
  if (attempt <= 1) return 0;
  // Double the wait for each attempt after the second, up to one minute.
  return Math.min(1000 * 2 ** (attempt - 2), 60000);
}
```

For the full list of status codes and what each one means, see the [status code reference](https://example.com/reference/status-codes). The failures that count are listed under [When to retry](#when-to-retry), and [document.txt](document.txt) has a short summary of these rules.
