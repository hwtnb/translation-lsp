import * as vscode from "vscode";
import {
  type Answer,
  type ChoiceRequest,
  type Closing,
  answerWith,
  awaitUnlessClosed,
} from "./choice";

/**
 * Shows `request` with VS Code's own parts until the user answers or
 * `closing` closes it: a menu as a Quick Pick, and a confirmation as a modal
 * warning, to which VS Code adds Cancel. The titles are the server's,
 * untranslated.
 */
export async function showChoice(request: ChoiceRequest, closing: Closing): Promise<Answer> {
  const items = request.choices.map((choice) => ({
    label: choice.title,
    title: choice.title,
    choice,
  }));
  if (request.kind === "menu") {
    const source = new vscode.CancellationTokenSource();
    const unlink = closing.onClose(() => source.cancel());
    try {
      const options = { placeHolder: request.message || undefined };
      const picked = await vscode.window.showQuickPick(items, options, source.token);
      return answerWith(closing.isClosed ? undefined : picked?.choice);
    } finally {
      unlink();
      source.dispose();
    }
  }

  const shown = vscode.window.showWarningMessage(request.message, { modal: true }, ...items);
  const picked = await awaitUnlessClosed(shown, closing.closed);
  if (closing.isClosed) {
    tellIgnoredConsent(shown);
    return null;
  }
  return answerWith(picked?.choice);
}

/**
 * Tells once that nothing was done if the user agrees in a dialog that stays
 * open after its request ended: VS Code cannot close a modal dialog.
 */
function tellIgnoredConsent(shown: PromiseLike<unknown>): void {
  void Promise.resolve(shown).then((picked) => {
    if (picked !== undefined) {
      void vscode.window.showInformationMessage(
        vscode.l10n.t(
          "Nothing was done: the language server stopped or withdrew the question before you answered.",
        ),
      );
    }
  });
}
