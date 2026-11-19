use std::{env, time::Duration};

use tokio::time::sleep;

use crate::{
    config::Settings,
    providers::{Intent, Output},
};

use super::{
    Capabilities, Kind, Translated, TranslationProvider, dictionary,
    error::{Error, Service},
};

#[derive(Clone, Default)]
pub(crate) struct MockOptions {
    pub delay: Duration,
    pub timeout_ms: Option<u64>,
    pub fail: bool,
    /// Automatic translations find the text already in the target language.
    pub same_language: bool,
}

impl MockOptions {
    pub fn apply_timeouts(&self, settings: &mut Settings) {
        if let Some(milliseconds) = self.timeout_ms {
            settings.hover_timeout_ms = milliseconds;
            settings.selection_timeout_ms = milliseconds;
            settings.document_timeout_ms = milliseconds;
        }
    }

    pub fn from_args() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut options = Self::default();
        let mut args = env::args().skip(1);
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--stdio" => (),
                "--mock-delay-ms" => {
                    let milliseconds: u64 = args.next().ok_or("Missing mock delay")?.parse()?;
                    if milliseconds > 30_000 {
                        return Err("Mock delay must be between 0 and 30000 milliseconds".into());
                    }
                    options.delay = Duration::from_millis(milliseconds);
                }
                "--mock-fail" => options.fail = true,
                "--mock-same-language" => options.same_language = true,
                "--mock-timeout-ms" => {
                    let milliseconds: u64 = args.next().ok_or("Missing mock timeout")?.parse()?;
                    if !(1..=30_000).contains(&milliseconds) {
                        return Err("Mock timeout must be between 1 and 30000 milliseconds".into());
                    }
                    // This fixture-only override caps both kinds of mock request.
                    options.timeout_ms = Some(milliseconds);
                }
                _ => return Err(format!("Unknown argument: {argument}").into()),
            }
        }
        Ok(options)
    }
}

/// The context of every mock translation: the mock provider has no account.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MockContext;

#[derive(Clone, Default)]
pub(crate) struct MockProvider {
    options: MockOptions,
}

impl MockProvider {
    pub fn new(options: MockOptions) -> Self {
        Self { options }
    }
}

const COMMENT: &str = "// Retry only if the remote service\n    // reports a temporary failure.";
const COMMENT_BODY: &str = "Retry only if the remote service\nreports a temporary failure.";
const DOCSTRING_BODY: &str = "Retry only if the remote service\n    reports a temporary failure.";
const DOXYGEN_BODY: &str = "@brief Retry only if the remote service\nreports a temporary failure.";
const XML_COMMENT_BODY: &str =
    "<summary>\nRetry only if the remote service\nreports a temporary failure.\n</summary>";
const PARAGRAPH: &str =
    "The `retry_count` value is preserved.\nRetry only after a temporary failure. 🚀";
/// The term of the Markdown sample that the mock provider looks up.
const TERM: &str = "retry_count";
const TERM_ENTRY: &str = r#"{"senses": [{"part_of_speech": "識別子", "meaning": "再試行の回数"}], "parts": [{"part": "retry", "meaning": "再試行"}, {"part": "count", "meaning": "回数"}], "combined": "再試行した回数", "example": {"text": "The retry_count value is preserved.", "translation": "retry_count の値は保持される。"}}"#;

/// The lines of `samples/long-document.md` that the mock provider translates
/// in parts, each with its translation. Blank lines stay blank.
const LONG_DOCUMENT: [(&str, &str); 31] = [
    ("# Connection pool", "# 接続プール"),
    (
        "This sample is long enough to be sent in parts when `document_split.enabled` is `true` and `document_split.chunk_kib` is `1`. Each part is translated on its own, and the tab opens only after every part is done.",
        "このサンプルは、`document_split.enabled` が `true` で `document_split.chunk_kib` が `1` のとき、部分に分けて送られる長さがある。各部分は別々に訳され、すべての部分が終わってからタブが開く。",
    ),
    ("## Opening connections", "## 接続を開く"),
    (
        "The pool opens a connection when a request needs one and none is free. It never opens more connections than `max_connections` allows, so a request may wait for another request to finish first.",
        "プールは、要求が接続を必要とし、空いている接続がないときに接続を開く。`max_connections` が許す数より多くは開かないので、要求はほかの要求が終わるのを待つことがある。",
    ),
    (
        "A new connection waits at most five seconds for the server to answer. When the server does not answer in time, the request fails, and the next request tries to open a connection again.",
        "新しい接続は、サーバーの応答を最大 5 秒待つ。サーバーが時間内に応答しないと要求は失敗し、次の要求がもう一度接続を開こうとする。",
    ),
    ("## Closing connections", "## 接続を閉じる"),
    (
        "This section is longer than twice the size of a part, so it is divided between its paragraphs, lists, tables and code blocks, and never inside one of them.",
        "この節は部分の大きさの 2 倍より長いので、段落、リスト、表、コードブロックの間で分けられ、それらの中では分けられない。",
    ),
    (
        "The pool closes a connection that nobody has used for the time that `idle_timeout` sets. It keeps at least the number of connections that `min_connections` sets, even when they are idle, so that the next request does not wait for a new connection.",
        "プールは、`idle_timeout` が決める時間だれも使っていない接続を閉じる。次の要求が新しい接続を待たずに済むよう、使われていなくても `min_connections` が決める数の接続は残す。",
    ),
    (
        "A connection is also closed in the following cases:",
        "接続は次の場合にも閉じる。",
    ),
    (
        "- The server closed its end of the connection.",
        "- サーバーが接続の自分の側を閉じた。",
    ),
    (
        "- A request on the connection failed with a network error.",
        "- 接続上の要求がネットワークのエラーで失敗した。",
    ),
    (
        "- The connection has been open for longer than `max_lifetime` allows.",
        "- 接続が `max_lifetime` の許す時間より長く開いている。",
    ),
    (
        "Before the pool closes a connection, it waits for the request on it to finish. A request never loses its connection in the middle of a response, even when the pool is shutting down.",
        "プールは接続を閉じる前に、その接続上の要求が終わるのを待つ。プールが終了する途中でも、要求が応答の途中で接続を失うことはない。",
    ),
    (
        "A connection that the server closed is not used again, even when it looks idle. The pool finds out only when it sends the next request on it, so that request is sent once more on a new connection, and fails only if the new connection fails too.",
        "サーバーが閉じた接続は、空いているように見えても使い直さない。プールがそれに気づくのは次の要求を送ったときなので、その要求は新しい接続でもう 1 回だけ送り、新しい接続でも失敗したときだけ失敗にする。",
    ),
    (
        "The function below closes the idle connections that the pool no longer needs. The line that starts with `#` inside the code block is a comment, not a heading, so the document is never divided there.",
        "下の関数は、プールがもう必要としない空いた接続を閉じる。コードブロックの中の `#` で始まる行は見出しではなくコメントなので、文書はそこでは分けられない。",
    ),
    ("```python", "```python"),
    ("def close_idle(pool, now):", "def close_idle(pool, now):"),
    (
        "    # Keep the minimum number of connections open.",
        "    # 最小の数の接続は開いたままにする。",
    ),
    (
        "    for connection in pool.idle()[pool.min_connections:]:",
        "    for connection in pool.idle()[pool.min_connections:]:",
    ),
    (
        "        if now - connection.last_used > pool.idle_timeout:",
        "        if now - connection.last_used > pool.idle_timeout:",
    ),
    (
        "            connection.close()",
        "            connection.close()",
    ),
    ("```", "```"),
    (
        "The table shows how long a connection stays open with the default settings.",
        "この表は、既定の設定で接続がどれだけ開いたままになるかを示す。",
    ),
    ("| Setting | Default | Effect |", "| 設定 | 既定 | 効果 |"),
    ("| --- | --- | --- |", "| --- | --- | --- |"),
    (
        "| `idle_timeout` | 10 minutes | Closes a connection that nobody uses |",
        "| `idle_timeout` | 10 分 | だれも使わない接続を閉じる |",
    ),
    (
        "| `max_lifetime` | 30 minutes | Closes a connection however busy it is |",
        "| `max_lifetime` | 30 分 | どれだけ忙しくても接続を閉じる |",
    ),
    (
        "| `min_connections` | 0 | Keeps this many connections open |",
        "| `min_connections` | 0 | この数の接続を開いたままにする |",
    ),
    (
        "When a connection closes because of `max_lifetime`, the pool opens a new one at once if the number of connections would fall below `min_connections`. Requests that are waiting for a connection take the new one in the order they arrived.",
        "`max_lifetime` のために接続が閉じたとき、接続の数が `min_connections` を下回るなら、プールはすぐに新しい接続を開く。接続を待っている要求は、着いた順に新しい接続を取る。",
    ),
    ("## Monitoring", "## 監視"),
    (
        "The pool reports the number of open connections, idle connections and waiting requests every minute. A number of waiting requests that keeps rising means that `max_connections` is too low for the load, and a number of idle connections that stays high means that `min_connections` is higher than the load needs.",
        "プールは、開いている接続、空いている接続、待っている要求の数を毎分報告する。待っている要求の数が増え続けるなら負荷に対して `max_connections` が小さすぎ、空いている接続の数が多いままなら `min_connections` が負荷に必要な数より大きい。",
    ),
];

impl TranslationProvider for MockProvider {
    type Context = MockContext;

    fn context(&self) -> Result<MockContext, Error> {
        Ok(MockContext)
    }

    fn capabilities() -> Capabilities {
        Capabilities {
            server_name: "Translation LSP (Mock)",
            automatic_selection: false,
            account_menu: None,
            commands: &[],
            hover_note: Some("Mock translation (fixed sample)"),
            comment_notice: None,
            range_output_bounded: false,
        }
    }

    async fn translate(
        &self,
        intent: Intent,
        _output: Output,
        text: &str,
        target_language: &str,
        context: MockContext,
    ) -> Result<Translated<MockContext>, Error> {
        sleep(self.options.delay).await;
        if self.options.fail {
            return Err(Error::failed("Simulated mock translation failure"));
        }
        if target_language != "ja" {
            return Err(Error::failed("The mock provider only targets Japanese"));
        }
        let translation = match text.replace("\r\n", "\n").replace('\r', "\n").trim() {
            COMMENT | COMMENT_BODY | DOCSTRING_BODY => {
                "リモートサービスが一時的な障害を報告した場合に限り、再試行する。"
            }
            DOXYGEN_BODY => {
                "@brief リモートサービスが一時的な障害を報告した場合に限り、再試行する。"
            }
            XML_COMMENT_BODY => {
                "<summary>\nリモートサービスが一時的な障害を報告した場合に限り、再試行する。\n</summary>"
            }
            PARAGRAPH => {
                "`retry_count` の値を維持する。\n一時的な障害が発生した場合に限り再試行する。🚀"
            }
            _ => {
                return Err(Error::failed(
                    "Mock translation only supports the bundled sample text",
                ));
            }
        };
        if self.options.same_language && intent == Intent::Automatic {
            return Ok(Translated {
                text: String::new(),
                context,
                kind: Kind::SameLanguage,
            });
        }
        Ok(Translated {
            text: translation.into(),
            context,
            kind: Kind::Translation,
        })
    }

    /// Translates a part of `samples/long-document.md` line by line, so that
    /// any part of it is translated.
    async fn translate_part(
        &self,
        text: &str,
        target_language: &str,
        context: MockContext,
    ) -> Result<Translated<MockContext>, Error> {
        sleep(self.options.delay).await;
        if self.options.fail {
            return Err(Error::failed("Simulated mock translation failure"));
        }
        if target_language != "ja" {
            return Err(Error::failed("The mock provider only targets Japanese"));
        }
        let lines = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .split('\n')
            .map(|line| {
                if line.trim().is_empty() {
                    return Some("");
                }
                LONG_DOCUMENT
                    .iter()
                    .find_map(|(source, translation)| (*source == line).then_some(*translation))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                Error::failed("Mock translation only supports the bundled sample text")
            })?;
        Ok(Translated {
            text: lines.join("\n"),
            context,
            kind: Kind::Translation,
        })
    }

    async fn look_up(
        &self,
        _intent: Intent,
        _output: Output,
        term: &str,
        _line: &str,
        target_language: &str,
        context: MockContext,
    ) -> Result<Translated<MockContext>, Error> {
        sleep(self.options.delay).await;
        if self.options.fail {
            return Err(Error::failed("Simulated mock translation failure"));
        }
        if target_language != "ja" || term != TERM {
            return Err(Error::failed(
                "The mock provider only looks up retry_count in Japanese",
            ));
        }
        dictionary::entry(term, TERM_ENTRY, Service::OpenAi, context)
    }
}

#[cfg(test)]
mod tests {
    use crate::providers::{Intent, Output};

    use super::*;
    #[tokio::test(flavor = "current_thread")]
    async fn arbitrary_text_is_not_presented_as_a_translation() {
        assert!(
            MockProvider::default()
                .translate(
                    Intent::Automatic,
                    Output::Range,
                    "Any text",
                    "ja",
                    MockContext
                )
                .await
                .is_err()
        );
        for text in [COMMENT, COMMENT_BODY, PARAGRAPH] {
            assert!(
                MockProvider::default()
                    .translate(
                        Intent::Automatic,
                        Output::Range,
                        &text.replace('\n', "\r\n"),
                        "ja",
                        MockContext
                    )
                    .await
                    .is_ok()
            );
        }
        assert!(
            MockProvider::default()
                .translate(
                    Intent::Automatic,
                    Output::Range,
                    COMMENT_BODY,
                    "en",
                    MockContext
                )
                .await
                .is_err()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn parts_of_the_long_sample_are_translated_line_by_line() {
        let part = "## Monitoring\n\n\n- The server closed its end of the connection.";
        for ending in ["\n", "\r\n", "\r"] {
            assert_eq!(
                MockProvider::default()
                    .translate_part(&part.replace('\n', ending), "ja", MockContext)
                    .await
                    .unwrap()
                    .text,
                "## 監視\n\n\n- サーバーが接続の自分の側を閉じた。"
            );
        }
        for (text, target) in [("## Monitoring\nAny text", "ja"), ("## Monitoring", "en")] {
            assert!(
                MockProvider::default()
                    .translate_part(text, target, MockContext)
                    .await
                    .is_err()
            );
        }
        // A whole text of the sample is not translated.
        assert!(
            MockProvider::default()
                .translate(
                    Intent::Explicit,
                    Output::Document,
                    "## Monitoring",
                    "ja",
                    MockContext
                )
                .await
                .is_err()
        );
    }
}
