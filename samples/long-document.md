# Connection pool

This sample is long enough to be sent in parts when `document_split.enabled` is `true` and `document_split.chunk_kib` is `1`. Each part is translated on its own, and the tab opens only after every part is done.

## Opening connections

The pool opens a connection when a request needs one and none is free. It never opens more connections than `max_connections` allows, so a request may wait for another request to finish first.

A new connection waits at most five seconds for the server to answer. When the server does not answer in time, the request fails, and the next request tries to open a connection again.

## Closing connections

This section is longer than twice the size of a part, so it is divided between its paragraphs, lists, tables and code blocks, and never inside one of them.

The pool closes a connection that nobody has used for the time that `idle_timeout` sets. It keeps at least the number of connections that `min_connections` sets, even when they are idle, so that the next request does not wait for a new connection.

A connection is also closed in the following cases:

- The server closed its end of the connection.
- A request on the connection failed with a network error.
- The connection has been open for longer than `max_lifetime` allows.

Before the pool closes a connection, it waits for the request on it to finish. A request never loses its connection in the middle of a response, even when the pool is shutting down.

A connection that the server closed is not used again, even when it looks idle. The pool finds out only when it sends the next request on it, so that request is sent once more on a new connection, and fails only if the new connection fails too.

The function below closes the idle connections that the pool no longer needs. The line that starts with `#` inside the code block is a comment, not a heading, so the document is never divided there.

```python
def close_idle(pool, now):
    # Keep the minimum number of connections open.
    for connection in pool.idle()[pool.min_connections:]:
        if now - connection.last_used > pool.idle_timeout:
            connection.close()
```

The table shows how long a connection stays open with the default settings.

| Setting | Default | Effect |
| --- | --- | --- |
| `idle_timeout` | 10 minutes | Closes a connection that nobody uses |
| `max_lifetime` | 30 minutes | Closes a connection however busy it is |
| `min_connections` | 0 | Keeps this many connections open |

When a connection closes because of `max_lifetime`, the pool opens a new one at once if the number of connections would fall below `min_connections`. Requests that are waiting for a connection take the new one in the order they arrived.

## Monitoring

The pool reports the number of open connections, idle connections and waiting requests every minute. A number of waiting requests that keeps rising means that `max_connections` is too low for the load, and a number of idle connections that stays high means that `min_connections` is higher than the load needs.
