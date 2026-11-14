// Retry only if the remote service
// reports a temporary failure.
const retry_count = 3;

/// The delay before the next retry, in milliseconds.
/// It doubles with each attempt.
int retryDelay(int attempt) => 200 * (1 << attempt);

/* A comment can hold /* a nested comment */ and go on. */
bool temporary(int status) => status == 503; // 503 means "try again later".

const separator = '// not a comment';
const raw = r'/* not a comment either */';
const quoted = '''
// not a comment in three quotes
''';
final message = '// not a comment: ${retryDelay(1) /* The delay of the first retry. */}';
