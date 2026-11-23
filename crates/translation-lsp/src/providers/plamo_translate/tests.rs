use serde_json::{Value, json};

use super::*;
use crate::providers::{
    anthropic::test_support::KeyDirectory,
    credential_directory,
    error::ErrorKind,
    http_fixture::{HttpFixture, Reply},
};

const KEY: &str = "sk-synthetic-local-key";
const SOURCE: &str = "Private source sentence.";

fn chunk(value: Value) -> String {
    format!("data: {value}\n\n")
}

/// A reply of `text` with `finish_reason`, and `[DONE]`.
fn finished(text: &str, reason: &str) -> String {
    format!(
        "{}{}data: [DONE]\n\n",
        chunk(
            json!({"choices": [{"index": 0, "delta": {"role": "assistant", "content": text}, "finish_reason": null}]})
        ),
        chunk(json!({"choices": [{"index": 0, "delta": {}, "finish_reason": reason}]})),
    )
}

fn completed(text: &str) -> String {
    finished(text, "stop")
}

/// The file of the key for the fixture's origin, which the OpenAI-compatible
/// provider uses as well.
fn file(fixture: &HttpFixture) -> String {
    let port = fixture.base.rsplit(':').next().unwrap();
    format!("api-key-http-127.0.0.1-{port}")
}

async fn translate(
    provider: &PlamoProvider,
    intent: Intent,
    output: Output,
    text: &str,
) -> Result<Translated<Option<KeyId>>, Error> {
    provider
        .translate(intent, output, text, "ja", provider.context()?)
        .await
}

#[tokio::test(flavor = "current_thread")]
async fn request_holds_the_settings_and_the_text_as_written() {
    let fixture = HttpFixture::new().await;
    let keys = KeyDirectory::new();
    let text = "Call `retry()`.\n\n<source_text>kept</source_text>";
    for (source_language, settings) in [
        ("auto", "[plamo-translate]\ntarget_lang=ja"),
        ("en", "[plamo-translate]\ntarget_lang=ja\nsource_lang=en"),
    ] {
        let provider = PlamoProvider::synthetic(
            &format!("{}/v1/", fixture.base),
            source_language,
            &keys.path,
        );
        for output in [Output::Range, Output::Document] {
            fixture.reply(Reply::stream(completed("訳文")));
            let translated = translate(&provider, Intent::Explicit, output, text)
                .await
                .unwrap();
            assert_eq!(translated.text, "訳文");
        }
        let records = fixture.records();
        let [.., range, document] = &records[..] else {
            panic!("Expected two requests")
        };
        for record in [range, document] {
            assert_eq!(record.path, "/v1/chat/completions");
            assert!(record.headers.get("authorization").is_none());
            let body = &record.body;
            assert_eq!(body["model"], "synthetic/plamo");
            assert_eq!(body["stream"], true);
            assert_eq!(body["temperature"], 0);
            assert_eq!(body["stop"], json!(["<|plamo:tag|>", "<|plamo:bos|>"]));
            assert_eq!(
                body["messages"],
                json!([
                    {"role": "system", "content": settings},
                    {"role": "user", "content": text},
                ])
            );
        }
        assert_eq!(range.body["max_tokens"], 16_384 + 4 * text.len());
        assert!(document.body.get("max_tokens").is_none());
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_key_of_the_origin_is_sent_when_one_is_saved() {
    let fixture = HttpFixture::new().await;
    let keys = KeyDirectory::new();
    let provider = PlamoProvider::synthetic(&format!("{}/v1", fixture.base), "auto", &keys.path);
    assert_eq!(provider.context().unwrap(), None);
    fixture.reply(Reply::stream(completed("訳")));
    translate(&provider, Intent::Explicit, Output::Range, SOURCE)
        .await
        .unwrap();
    credential_directory::replace(&keys.path, &file(&fixture), KEY.as_bytes()).unwrap();
    assert!(provider.context().unwrap().is_some());
    fixture.reply(Reply::stream(completed("訳")));
    translate(&provider, Intent::Explicit, Output::Range, SOURCE)
        .await
        .unwrap();
    let records = fixture.records();
    assert!(records[0].headers.get("authorization").is_none());
    assert_eq!(records[1].headers["authorization"], format!("Bearer {KEY}"));
}

/// The reply is the translation: nothing is taken out of it, not even the
/// tags that the prompt of other models asks for.
#[tokio::test(flavor = "current_thread")]
async fn the_reply_is_used_as_written() {
    let fixture = HttpFixture::new().await;
    let keys = KeyDirectory::new();
    let provider = PlamoProvider::synthetic(&format!("{}/v1", fixture.base), "auto", &keys.path);
    for reply in [
        "<source_text>訳文</source_text>",
        "<languages>en</languages>\n訳文",
    ] {
        fixture.reply(Reply::stream(completed(reply)));
        let translated = translate(&provider, Intent::Automatic, Output::Range, SOURCE)
            .await
            .unwrap();
        assert_eq!(
            (translated.kind, translated.text.as_str()),
            (Kind::Translation, reply)
        );
    }
}

/// Only an automatic request with `source_language` set to `auto` reads a
/// reply that repeats the text as text already in the target language.
#[tokio::test(flavor = "current_thread")]
async fn only_automatic_requests_from_auto_read_a_repeated_text_as_the_same_language() {
    let text = "  これは日本語です。\n";
    for (intent, source_language, reply, kind) in [
        (
            Intent::Automatic,
            "auto",
            "これは日本語です。",
            Kind::SameLanguage,
        ),
        (
            Intent::Automatic,
            "auto",
            "\nこれは日本語です。  ",
            Kind::SameLanguage,
        ),
        (
            Intent::Automatic,
            "auto",
            "これは日本語です",
            Kind::Translation,
        ),
        (
            Intent::Explicit,
            "auto",
            "これは日本語です。",
            Kind::Translation,
        ),
        (
            Intent::Automatic,
            "en",
            "これは日本語です。",
            Kind::Translation,
        ),
    ] {
        let fixture = HttpFixture::new().await;
        let keys = KeyDirectory::new();
        let provider =
            PlamoProvider::synthetic(&format!("{}/v1", fixture.base), source_language, &keys.path);
        fixture.reply(Reply::stream(completed(reply)));
        let translated = translate(&provider, intent, Output::Range, text)
            .await
            .unwrap();
        let shown = if kind == Kind::SameLanguage {
            ""
        } else {
            reply
        };
        assert_eq!(
            (translated.kind, translated.text.as_str()),
            (kind, shown),
            "{intent:?} {source_language} {reply:?}"
        );
    }
}

/// A term is translated like any other text, so an automatic one that comes
/// back as it was sent shows nothing. A part is translated like an explicit
/// request, whatever comes back.
#[tokio::test(flavor = "current_thread")]
async fn a_term_is_translated_like_a_text_and_a_part_like_an_explicit_request() {
    let fixture = HttpFixture::new().await;
    let keys = KeyDirectory::new();
    let provider = PlamoProvider::synthetic(&format!("{}/v1", fixture.base), "auto", &keys.path);
    let context = provider.context().unwrap();
    for (intent, kind, shown) in [
        (Intent::Explicit, Kind::Translation, "retry_count"),
        (Intent::Automatic, Kind::SameLanguage, ""),
    ] {
        fixture.reply(Reply::stream(completed("retry_count")));
        let term = provider
            .look_up(
                intent,
                Output::Range,
                "retry_count",
                "Set retry_count here.",
                "ja",
                context,
            )
            .await
            .unwrap();
        assert_eq!((term.kind, term.text.as_str()), (kind, shown), "{intent:?}");
    }
    fixture.reply(Reply::stream(completed(SOURCE)));
    let part = provider
        .translate_part(SOURCE, "ja", context)
        .await
        .unwrap();
    assert_eq!((part.kind, part.text.as_str()), (Kind::Translation, SOURCE));
    let records = fixture.records();
    for record in &records[..2] {
        assert_eq!(record.body["messages"][1]["content"], "retry_count");
    }
    assert_eq!(records[2].body["messages"][1]["content"], SOURCE);
    assert!(records[2].body.get("max_tokens").is_none());
}

/// A reply of only whitespace is no translation, whatever the request.
#[tokio::test(flavor = "current_thread")]
async fn a_reply_of_only_whitespace_is_an_error() {
    let fixture = HttpFixture::new().await;
    let keys = KeyDirectory::new();
    for source_language in ["auto", "en"] {
        let provider =
            PlamoProvider::synthetic(&format!("{}/v1", fixture.base), source_language, &keys.path);
        for intent in [Intent::Automatic, Intent::Explicit] {
            for output in [Output::Range, Output::Document] {
                fixture.reply(Reply::stream(completed(" \n\t ")));
                let error = translate(&provider, intent, output, SOURCE)
                    .await
                    .err()
                    .unwrap();
                assert_eq!(error.kind(), ErrorKind::Failed, "{error}");
                assert_eq!(
                    error.to_string(),
                    "PLaMo Translate completed without translation text"
                );
            }
        }
    }
    let provider = PlamoProvider::synthetic(&format!("{}/v1", fixture.base), "auto", &keys.path);
    fixture.reply(Reply::stream(completed("\n")));
    let error = provider
        .translate_part(SOURCE, "ja", provider.context().unwrap())
        .await
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::Failed, "{error}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_reply_cut_short_or_refused_is_an_error_without_the_text() {
    let fixture = HttpFixture::new().await;
    let keys = KeyDirectory::new();
    credential_directory::replace(&keys.path, &file(&fixture), KEY.as_bytes()).unwrap();
    let provider = PlamoProvider::synthetic(&format!("{}/v1", fixture.base), "auto", &keys.path);
    fixture.reply(Reply::stream(finished("部分", "length")));
    fixture.reply(Reply::failure(
        400,
        json!({"error": {"message": "the request exceeds the available context size", "type": "invalid_request_error"}}).to_string(),
    ));
    for message in [
        "PLaMo Translate response stopped at the output token limit; no partial translation was saved",
        "PLaMo Translate HTTP 400: the request exceeds the available context size (code=invalid_request_error)",
    ] {
        let error = translate(&provider, Intent::Explicit, Output::Document, SOURCE)
            .await
            .err()
            .unwrap();
        assert_eq!(error.kind(), ErrorKind::Failed, "{error}");
        let error = error.to_string();
        assert!(error.starts_with(message), "{error}");
        assert!(!error.contains(SOURCE) && !error.contains(KEY), "{error}");
    }
}

/// The text is echoed back in an error, which shows it only redacted.
#[tokio::test(flavor = "current_thread")]
async fn an_error_that_repeats_the_text_does_not_show_it() {
    let fixture = HttpFixture::new().await;
    let keys = KeyDirectory::new();
    let provider = PlamoProvider::synthetic(&format!("{}/v1", fixture.base), "auto", &keys.path);
    fixture.reply(Reply::failure(
        400,
        json!({"error": {"message": format!("cannot read {SOURCE}")}}).to_string(),
    ));
    let error = translate(&provider, Intent::Explicit, Output::Range, SOURCE)
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(error.starts_with("PLaMo Translate HTTP 400: "), "{error}");
    assert!(!error.contains(SOURCE), "{error}");
}

#[test]
fn the_identity_names_the_provider_service_and_model() {
    let keys = KeyDirectory::new();
    let provider = PlamoProvider::synthetic("http://127.0.0.1:11434/v1/", "auto", &keys.path);
    assert_eq!(
        provider.identity(Output::Range),
        Identity::new("plamo_translate")
            .with("base_url", "http://127.0.0.1:11434/v1")
            .with("model", "synthetic/plamo")
    );
    assert_eq!(
        provider.identity(Output::Range),
        provider.identity(Output::Document)
    );
}
