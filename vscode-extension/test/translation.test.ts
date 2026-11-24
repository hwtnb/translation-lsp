import * as vscode from "vscode";
import { expect, test, vi } from "vitest";
import { type Shown, TranslationThreads } from "../src/translation";

const { createCommentThread, env } = vi.hoisted(() => ({
  createCommentThread: vi.fn(
    (_uri: vscode.Uri, _range: vscode.Range, _comments: vscode.Comment[]) => ({
      dispose: vi.fn(),
    }),
  ),
  env: { language: "en" },
}));

vi.mock("vscode", () => ({
  comments: { createCommentController: () => ({ createCommentThread, dispose: vi.fn() }) },
  workspace: {
    textDocuments: [{ uri: "file:///source", version: 1 }],
    onDidChangeTextDocument: () => ({ dispose: vi.fn() }),
    onDidCloseTextDocument: () => ({ dispose: vi.fn() }),
  },
  env,
  Uri: { parse: (uri: string) => uri },
  Range: vi.fn(class {}),
  MarkdownString: vi.fn(class {}),
  CommentMode: { Preview: 1 },
  CommentThreadCollapsibleState: { Expanded: 1 },
}));

vi.mock("vscode-languageclient/node", () => ({}));

test.each([
  { failed: false, locale: "en", targetLanguage: "ja", name: "Translation LSP: Japanese" },
  { failed: true, locale: "en", targetLanguage: "ja", name: "Translation LSP: Japanese" },
  { failed: false, locale: "ja", targetLanguage: "ja", name: "Translation LSP: 日本語" },
  { failed: true, locale: "ja", targetLanguage: "ja", name: "Translation LSP: 日本語" },
  { failed: false, locale: "ja", targetLanguage: "qaa", name: "Translation LSP: qaa" },
])("uses $name as the author (failed=$failed)", ({ failed, locale, targetLanguage, name }) => {
  createCommentThread.mockClear();
  env.language = locale;
  const shown: Shown = {
    uri: "file:///source",
    version: 1,
    range: { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } },
    failed,
    markdown: "Result",
    text: "Result",
    actions: [],
    targetLanguage,
  };
  const threads = new TranslationThreads(vi.fn());

  try {
    threads.show(shown);
    expect(createCommentThread).toHaveBeenLastCalledWith(expect.anything(), expect.anything(), [
      expect.objectContaining({ author: { name } }),
    ]);
  } finally {
    threads.dispose();
  }
});
