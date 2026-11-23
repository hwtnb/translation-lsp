//! The streamed replies and the errors of a Chat Completions service.
use serde_json::{Value, json};

use std::borrow::Cow;

use super::Names;
use crate::{
    config::limits::{SizeLimit, SizeSetting},
    providers::{
        error::{Cause, Error, RemoteError, RemoteKind},
        responses::request_id,
        sse,
        transport::{self, safe},
    },
};

const NOT_FOUND: &str = "Check provider.base_url and the model names";

/// The error a failed response or a stream error reports. Services shape
/// their errors in several ways, so the body is read leniently: Gemini, for
/// one, puts the error in a list.
/// `rejected` is what to do when the service refuses the request.
fn failure(
    names: Names,
    status: u16,
    body: &Value,
    request_id: Option<&str>,
    secrets: &[&str],
    rejected: &str,
) -> Error {
    let body = body
        .as_array()
        .and_then(|list| list.first())
        .unwrap_or(body);
    let error = body
        .get("error")
        .filter(|error| error.is_object())
        .unwrap_or(body);
    // Some services give the code as a number.
    let code = match error.get("code") {
        Some(Value::String(code)) => Some(code.clone()),
        Some(Value::Number(code)) => Some(code.to_string()),
        _ => error.get("type").and_then(Value::as_str).map(str::to_owned),
    };
    let detail = error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| body.get("error").and_then(Value::as_str))
        .or_else(|| body.as_str())
        .filter(|detail| !detail.trim().is_empty());
    let failed = format!("{} request failed", names.short);
    let detail = detail.unwrap_or(&failed);
    // An error in a stream comes after HTTP 200. Services that number their
    // codes, such as OpenRouter, give the status of the error there.
    let class = match error.get("code").and_then(Value::as_u64) {
        Some(code) if status == 200 => u16::try_from(code).unwrap_or(status),
        _ => status,
    };
    let (kind, advice) = match (class, code.as_deref()) {
        (401, _) => (
            RemoteKind::Configuration,
            Some(Cow::Owned(rejected.to_owned())),
        ),
        (402, _) => (RemoteKind::UsageLimit, None),
        (403, _) => (RemoteKind::NotEligible, None),
        (404, _) => (RemoteKind::Configuration, Some(Cow::Borrowed(NOT_FOUND))),
        (_, Some("rate_limit_exceeded" | "server_error" | "server_is_overloaded")) => {
            (RemoteKind::Temporary, None)
        }
        (408 | 429, _) => (RemoteKind::Temporary, None),
        (class, _) if class >= 500 => (RemoteKind::Temporary, None),
        _ => (RemoteKind::Failed, None),
    };
    RemoteError {
        service: names.service,
        kind,
        status,
        detail: safe(detail, secrets, 512),
        code: code.map(|value| safe(&value, secrets, 128)),
        param: error
            .get("param")
            .and_then(Value::as_str)
            .map(|value| safe(value, secrets, 128)),
        request_id: request_id.map(|value| safe(value, secrets, 128)),
        advice,
    }
    .into()
}

pub(super) async fn failure_of(
    response: reqwest::Response,
    names: Names,
    secrets: &[&str],
    rejected: &str,
) -> Error {
    let status = response.status().as_u16();
    let request_id = request_id(response.headers()).map(str::to_owned);
    match transport::error_body(response, names.service).await {
        Ok(body) => failure(
            names,
            status,
            &body,
            request_id.as_deref(),
            secrets,
            rejected,
        ),
        Err(error) => error,
    }
}

/// The reply so far, from the first choice.
struct Reply {
    names: Names,
    text: String,
    finish_reason: Option<String>,
    done: bool,
}

impl Reply {
    fn new(names: Names) -> Self {
        Self {
            names,
            text: String::new(),
            finish_reason: None,
            done: false,
        }
    }

    /// Takes the event `data`, and `value`, its JSON if it is JSON.
    fn accept(
        &mut self,
        data: &str,
        value: Option<Value>,
        output_limit: SizeLimit,
    ) -> Result<(), Error> {
        if self.done {
            return Err(Error::protocol(
                self.names.service,
                "stream continued after [DONE]",
            ));
        }
        if data.trim() == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let value = value.ok_or_else(|| {
            Error::protocol(self.names.service, "SSE event contains invalid JSON")
        })?;
        // A chunk without choices, such as one that only reports usage, adds
        // no text.
        let Some(choices) = value.get("choices").and_then(Value::as_array) else {
            return Ok(());
        };
        for choice in choices {
            if choice.get("index").and_then(Value::as_u64).unwrap_or(0) != 0 {
                continue;
            }
            // Reasoning comes in fields of its own and is not shown.
            let text = choice["delta"]["content"].as_str().unwrap_or_default();
            if let Some(finished) = &self.finish_reason {
                // OpenRouter repeats the finished choice, without text, in the
                // chunk that reports usage.
                if text.is_empty()
                    && choice["finish_reason"]
                        .as_str()
                        .is_none_or(|reason| reason == finished)
                {
                    continue;
                }
                return Err(Error::protocol(
                    self.names.service,
                    "choice continued after it finished",
                ));
            }
            output_limit.check(
                self.text.len().saturating_add(text.len()),
                SizeSetting::Output,
            )?;
            self.text.push_str(text);
            if let Some(reason) = choice["finish_reason"].as_str() {
                self.finish_reason = Some(reason.to_owned());
            }
        }
        Ok(())
    }

    /// Only the transport reader may call this, after EOF and truncation
    /// checks. A stream that gave its reason to finish is complete even
    /// without `[DONE]`, which some services leave out.
    fn finish(self, secrets: &[&str]) -> Result<String, Error> {
        let service = self.names.service;
        match self.finish_reason.as_deref() {
            Some("stop") if !self.text.is_empty() => Ok(self.text),
            Some("stop") => Err(Error::failed(format!(
                "{service} completed without translation text"
            ))),
            Some("length") => Err(Error::failed(format!(
                "{service} response stopped at the output token limit; no partial translation was saved"
            ))),
            Some("content_filter") => Err(Cause::Refusal {
                service,
                category: Some("content_filter".into()),
            }
            .into()),
            Some(reason) => Err(Error::failed(format!(
                "{service} response ended without completing the translation (finish_reason={}); no partial translation was saved",
                safe(reason, secrets, 64)
            ))),
            None => Err(Error::failed(format!(
                "{service} stream ended without finish_reason; no partial translation was saved"
            ))),
        }
    }
}

pub(super) async fn text(
    response: reqwest::Response,
    names: Names,
    secrets: &[&str],
    rejected: &str,
    output_limit: SizeLimit,
) -> Result<String, Error> {
    let status = response.status().as_u16();
    let request_id = request_id(response.headers()).map(str::to_owned);
    let mut reply = Reply::new(names);
    sse::read_events(response, names.service, output_limit, |event| {
        let value = serde_json::from_str::<Value>(&event.data).ok();
        // A service that fails after it has begun says so in a chunk.
        if let Some(error) = value
            .as_ref()
            .and_then(|value| value.get("error"))
            .filter(|error| !error.is_null())
        {
            let error = failure(
                names,
                status,
                &json!({"error": error}),
                request_id.as_deref(),
                secrets,
                rejected,
            );
            return Err(error.in_stream(names.service, "error".into(), String::new()));
        }
        reply.accept(&event.data, value, output_limit)
    })
    .await?;
    reply.finish(secrets)
}

#[cfg(test)]
mod tests;
