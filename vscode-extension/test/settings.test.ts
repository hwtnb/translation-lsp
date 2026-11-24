import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { expect, test } from "vitest";
import {
  PROVIDER_FIELDS,
  checkServerPath,
  buildInitializationOptions,
  toServerName,
  listServerSettings,
  pickUserValue,
  toVscodeName,
} from "../src/settings";

/** A `Read` that gives the values of `settings` and nothing else. */
function readFrom(settings: Record<string, unknown>) {
  return (name: string) => settings[name];
}

test("names convert both ways", () => {
  expect(toVscodeName("translation_limits.input_kib.hover")).toBe(
    "translationLimits.inputKib.hover",
  );
  expect(toVscodeName("provider.base_url")).toBe("provider.baseUrl");
  for (const name of listServerSettings()) {
    expect(toVscodeName(toServerName(name))).toBe(name);
    expect(toServerName(name)).toMatch(/^[a-z0-9_.]+$/);
  }
});

test("only the settings the user wrote are sent", () => {
  expect(buildInitializationOptions(readFrom({}))).toStrictEqual({});
  expect(
    buildInitializationOptions(
      readFrom({
        targetLanguage: "fr",
        sourceLanguage: "",
        hoverTimeoutMs: 5000,
        "features.commentHover": false,
        "features.languages": { yaml: { comment_hover: false } },
        "translationLimits.inputKib.hover": 32,
        "translationLimits.work.hoverWaiters": 4,
        "diskCache.enabled": true,
      }),
    ),
  ).toStrictEqual({
    target_language: "fr",
    hover_timeout_ms: 5000,
    features: { comment_hover: false, languages: { yaml: { comment_hover: false } } },
    translation_limits: { input_kib: { hover: 32 }, work: { hover_waiters: 4 } },
    disk_cache: { enabled: true },
  });
});

test("a provider gets only its own settings", () => {
  const written = {
    "provider.baseUrl": "http://localhost:11434/v1",
    "provider.model": "a-model",
    "provider.effort": "medium",
    "provider.thinking": "off",
    "provider.speed": "fast",
  };

  expect(
    buildInitializationOptions(readFrom({ ...written, "provider.name": "chatgpt" })).provider,
  ).toStrictEqual({
    name: "chatgpt",
    model: "a-model",
    effort: "medium",
    speed: "fast",
  });
  expect(
    buildInitializationOptions(readFrom({ ...written, "provider.name": "anthropic_api" })).provider,
  ).toStrictEqual({
    name: "anthropic_api",
    model: "a-model",
    effort: "medium",
    thinking: false,
    speed: "fast",
  });
  expect(
    buildInitializationOptions(readFrom({ ...written, "provider.name": "openai_compatible" }))
      .provider,
  ).toStrictEqual({
    name: "openai_compatible",
    base_url: "http://localhost:11434/v1",
    model: "a-model",
    effort: "medium",
  });
  expect(
    buildInitializationOptions(readFrom({ ...written, "provider.name": "deepl" })).provider,
  ).toStrictEqual({
    name: "deepl",
  });
  // A provider that the settings do not list, such as the mock provider, is sent by name.
  expect(
    buildInitializationOptions(readFrom({ ...written, "provider.name": "mock" })).provider,
  ).toStrictEqual({
    name: "mock",
  });
  expect(buildInitializationOptions(readFrom({ ...written, "provider.name": "" })).provider).toBe(
    undefined,
  );
  expect(buildInitializationOptions(readFrom(written)).provider).toBe(undefined);
});

test("thinking is left to the server unless on or off", () => {
  const sendThinking = (value: unknown) =>
    buildInitializationOptions(
      readFrom({ "provider.name": "anthropic_api", "provider.thinking": value }),
    ).provider;
  expect(sendThinking("on")).toStrictEqual({ name: "anthropic_api", thinking: true });
  expect(sendThinking("off")).toStrictEqual({ name: "anthropic_api", thinking: false });
  expect(sendThinking("auto")).toStrictEqual({ name: "anthropic_api" });
  expect(sendThinking(undefined)).toStrictEqual({ name: "anthropic_api" });
});

test("the document settings are sent only with their model", () => {
  const document = {
    "provider.name": "anthropic_api",
    "provider.document.effort": "high",
    "provider.document.thinking": "on",
    "provider.document.speed": "standard",
  };

  expect(buildInitializationOptions(readFrom(document)).provider).toStrictEqual({
    name: "anthropic_api",
  });
  expect(
    buildInitializationOptions(readFrom({ ...document, "provider.document.model": "" })).provider,
  ).toStrictEqual({ name: "anthropic_api" });
  expect(
    buildInitializationOptions(readFrom({ ...document, "provider.document.model": "tab-model" }))
      .provider,
  ).toStrictEqual({
    name: "anthropic_api",
    document: { model: "tab-model", effort: "high", thinking: true, speed: "standard" },
  });
});

test("every provider field has a setting", () => {
  const names = listServerSettings();
  for (const [provider, fields] of Object.entries(PROVIDER_FIELDS)) {
    for (const field of fields) {
      expect(names.includes(toVscodeName(`provider.${field}`)), `${provider}: ${field}`).toBe(true);
    }
  }
});

test("a setting written in both places sends the value that VS Code makes of them", () => {
  const user = { rust: { comment_hover: false } };
  const workspace = { yaml: { comment_hover: false } };
  // VS Code merges the objects of the two places into the value it gives.
  const merged = { ...user, ...workspace };
  const read = (name: string) =>
    name === "features.languages"
      ? pickUserValue({ globalValue: user, workspaceValue: workspace, value: merged })
      : pickUserValue({ value: name === "targetLanguage" ? "ja" : undefined });
  expect(buildInitializationOptions(read)).toStrictEqual({ features: { languages: merged } });
  expect(pickUserValue({ workspaceValue: "fr", value: "fr" })).toBe("fr");
  expect(pickUserValue({ globalValue: "fr", workspaceValue: "de", value: "de" })).toBe("de");
  // A default that nobody wrote is not sent.
  expect(pickUserValue({ value: "ja" })).toBe(undefined);
});

test("a provider name that is no row of the table is sent alone", () => {
  for (const name of ["constructor", "__proto__", "toString", "hasOwnProperty"]) {
    const options = buildInitializationOptions(
      readFrom({ "provider.name": name, "provider.model": "a-model" }),
    );
    expect(options.provider, name).toStrictEqual({ name });
  }
});

test("the server path must be the absolute path of a file", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "translation-lsp-path-"));
  try {
    const file = path.join(directory, "translation-lsp");
    fs.writeFileSync(file, "");

    expect(checkServerPath(file)).toBe(undefined);
    expect(checkServerPath("")).toBe("empty");

    // Settings written in `settings.json` may have any type.
    for (const value of [123, [file], { path: file }, null, true]) {
      expect(checkServerPath(value), JSON.stringify(value)).toBe("not text");
    }

    expect(checkServerPath("translation-lsp")).toBe("relative");
    expect(checkServerPath(path.join(".", "bin", "translation-lsp"))).toBe("relative");
    expect(checkServerPath(path.join(directory, "missing"))).toBe("missing");
    expect(checkServerPath(directory)).toBe("not a file");

    // A loop of links, and a folder that may not be read, cannot be checked.
    const loop = path.join(directory, "loop");
    fs.symlinkSync(loop, loop);
    expect(checkServerPath(loop)).toBe("unreadable");

    const closed = path.join(directory, "closed");
    fs.mkdirSync(closed);
    fs.writeFileSync(path.join(closed, "translation-lsp"), "");
    fs.chmodSync(closed, 0o000);
    try {
      // A user who may read every folder, such as root, reads this one.
      const readable = (() => {
        try {
          fs.readdirSync(closed);
          return true;
        } catch {
          return false;
        }
      })();
      if (!readable) {
        expect(checkServerPath(path.join(closed, "translation-lsp"))).toBe("unreadable");
      }
    } finally {
      fs.chmodSync(closed, 0o700);
    }
  } finally {
    fs.rmSync(directory, { recursive: true });
  }
});
