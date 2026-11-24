import * as vscode from "vscode";
import { ErrorCodes, LSPErrorCodes, ResponseError } from "vscode-languageclient/node";
import { nameLanguage } from "./language";

/** The method of the notification that hands a translation over. */
export const SHOW_TRANSLATION = "translation-lsp/showTranslation";

/** A button that the server offers with a translation. */
export interface Action {
  id: string;
  title: string;
  command: string;
  arguments?: unknown[];
}

/** What `translation-lsp/showTranslation` carries. */
export interface Shown {
  uri: string;
  /** The version of the document that the translation belongs to. */
  version: number;
  range: {
    start: { line: number; character: number };
    end: { line: number; character: number };
  };
  failed: boolean;
  markdown: string;
  text: string;
  actions: Action[];
  /** The `target_language` setting of the server, as written. */
  targetLanguage: string;
}

/** The actions that have buttons of their own. Any other is offered under More Actions. */
const BUTTONS = ["retry", "delete"];

/** Runs a server command, as `workspace/executeCommand` does. */
export type Execute = (command: string, args: unknown[] | undefined) => Promise<unknown>;

/**
 * Shows a translation in a comment thread under its range, one at a time, with
 * buttons that copy it, run the server's actions, and close it. An edit of its
 * document closes it.
 */
export class TranslationThreads implements vscode.Disposable {
  private readonly controller = vscode.comments.createCommentController(
    "translation-lsp",
    "Translation LSP",
  );
  private readonly subscriptions: vscode.Disposable[] = [];
  private open?: { thread: vscode.CommentThread; shown: Shown };

  constructor(private readonly execute: Execute) {
    this.subscriptions.push(
      vscode.workspace.onDidChangeTextDocument((event) => {
        if (event.contentChanges.length > 0 && this.isOpenFor(event.document.uri)) {
          this.close();
        }
      }),
      vscode.workspace.onDidCloseTextDocument((document) => {
        if (this.isOpenFor(document.uri)) {
          this.close();
        }
      }),
    );
  }

  /**
   * Opens `shown` under its range, if its document is open at the version
   * that it belongs to: one that came after an edit belongs to no text.
   */
  show(shown: Shown): void {
    const uri = vscode.Uri.parse(shown.uri);
    const document = vscode.workspace.textDocuments.find(
      (document) => document.uri.toString() === uri.toString(),
    );
    if (document?.version !== shown.version) {
      return;
    }

    this.close();
    const { start, end } = shown.range;
    const range = new vscode.Range(start.line, start.character, end.line, end.character);
    const body = new vscode.MarkdownString(shown.markdown, true);
    // VS Code gives every comment a line for its author, which tells what
    // the translation is.
    const author = { name: describeTranslation(shown) };
    const comment: vscode.Comment = { body, mode: vscode.CommentMode.Preview, author };
    const thread = this.controller.createCommentThread(uri, range, [comment]);
    thread.canReply = false;
    thread.label = "";

    const ids = shown.actions.map((action) => action.id);
    const more = ids.some((id) => !BUTTONS.includes(id));
    thread.contextValue = [
      ...ids.filter((id) => BUTTONS.includes(id)),
      ...(more ? ["more"] : []),
    ].join(" ");
    thread.collapsibleState = vscode.CommentThreadCollapsibleState.Expanded;
    this.open = { thread, shown };
  }

  async copy(): Promise<void> {
    if (this.open) {
      await vscode.env.clipboard.writeText(this.open.shown.text);
    }
  }

  /** Runs the action `id`, or one of those without a button of their own if `id` is `undefined`. */
  async act(id: string | undefined): Promise<void> {
    const actions = this.open?.shown.actions ?? [];
    let action = actions.find((action) => action.id === id);
    if (id === undefined) {
      const others = actions.filter((action) => !BUTTONS.includes(action.id));
      const picked = await vscode.window.showQuickPick(
        others.map((action) => ({ label: action.title, action })),
        { placeHolder: vscode.l10n.t("Choose an action") },
      );
      action = picked?.action;
    }
    if (!action) {
      return;
    }

    this.close();
    try {
      await this.execute(action.command, action.arguments);
    } catch (error) {
      tellUntold(error);
    }
  }

  private isOpenFor(uri: vscode.Uri): boolean {
    return this.open?.thread.uri.toString() === uri.toString();
  }

  close(): void {
    this.open?.thread.dispose();
    this.open = undefined;
  }

  dispose(): void {
    this.close();
    this.controller.dispose();
    for (const subscription of this.subscriptions) {
      subscription.dispose();
    }
  }
}

/** What `shown` is: a translation into its target language, or a failure of one. */
function describeTranslation(shown: Shown): string {
  const language = nameLanguage(shown.targetLanguage ?? "", vscode.env.language);
  return shown.failed
    ? vscode.l10n.t("Translation to {0} failed", language)
    : vscode.l10n.t("Translation to {0}", language);
}

/**
 * Shows `error` of a server command, unless the server told the user of it
 * itself: it does so for every error of a command but a canceled one and one
 * whose text changed. The errors that the client makes, such as of the
 * connection, reach the user only here.
 */
export function tellUntold(error: unknown): void {
  const toldByServer =
    error instanceof ResponseError &&
    ![
      LSPErrorCodes.ContentModified,
      LSPErrorCodes.RequestCancelled,
      ErrorCodes.MessageWriteError,
      ErrorCodes.MessageReadError,
      ErrorCodes.PendingResponseRejected,
      ErrorCodes.ConnectionInactive,
    ].includes(error.code);
  if (!toldByServer) {
    void vscode.window.showErrorMessage(error instanceof Error ? error.message : String(error));
  }
}
