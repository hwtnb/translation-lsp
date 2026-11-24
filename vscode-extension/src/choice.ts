// What the server asks with `translation-lsp/choose`, and the answer to it.
// Nothing here uses the VS Code API, so the tests run it without VS Code.

/** The request with which the server asks the user to pick one of its choices. */
export const CHOOSE = "translation-lsp/choose";

/** One choice of a request, with the ID that the answer names it by. */
export interface Choice {
  id: string;
  title: string;
}

/**
 * What `translation-lsp/choose` carries: a menu that the user opened, or
 * agreement asked before an operation of the user goes on.
 */
export interface ChoiceRequest {
  kind: "menu" | "confirm";
  message: string;
  choices: Choice[];
}

/** The answer to a request: the ID of the choice picked, or `null` for none. */
export type Answer = { id: string } | null;

/** `params` as a request to choose, or `undefined` if it has another shape. */
export function readChoiceRequest(params: unknown): ChoiceRequest | undefined {
  if (typeof params !== "object" || params === null) {
    return undefined;
  }

  const { kind, message, choices } = params as Record<string, unknown>;
  if ((kind !== "menu" && kind !== "confirm") || typeof message !== "string") {
    return undefined;
  }
  if (!Array.isArray(choices) || !choices.every(isChoice)) {
    return undefined;
  }
  return { kind, message, choices: choices.map(({ id, title }) => ({ id, title })) };
}

function isChoice(value: unknown): value is Choice {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const { id, title } = value as Record<string, unknown>;
  return typeof id === "string" && typeof title === "string";
}

/** The answer that names `choice`, or `null` if none was picked. */
export function answerWith(choice: Choice | undefined): Answer {
  return choice ? { id: choice.id } : null;
}

/**
 * What `shown` gives, or `undefined` as soon as `closed` settles: a part of
 * VS Code that the extension cannot close, such as a modal dialog, answers
 * nothing once it should have closed.
 */
export function awaitUnlessClosed<T>(
  shown: PromiseLike<T>,
  closed: Promise<void>,
): Promise<T | undefined> {
  return Promise.race([Promise.resolve(shown), closed.then(() => undefined)]);
}

/** A token that tells when a request was canceled, as VS Code and the library give it. */
export interface Cancellation {
  readonly isCancellationRequested: boolean;
  onCancellationRequested(listener: () => void): { dispose(): void };
}

/** A signal that closes a choice once, and stays closed. */
export class Closing {
  private done = false;
  private listeners: (() => void)[] = [];
  /** Settles when the signal closes. */
  readonly closed: Promise<void> = new Promise((resolve) => this.listeners.push(resolve));

  get isClosed(): boolean {
    return this.done;
  }

  close(): void {
    if (this.done) {
      return;
    }
    this.done = true;
    for (const listener of this.listeners.splice(0)) {
      listener();
    }
  }

  /** Calls `listener` when the signal closes, until the returned function is called. */
  onClose(listener: () => void): () => void {
    this.listeners.push(listener);
    return () => {
      this.listeners = this.listeners.filter((other) => other !== listener);
    };
  }
}

/**
 * The choices open on the connection to a server. They close when their
 * request is canceled or the connection ends, and the next connection opens
 * its own.
 */
export class OpenChoices {
  private connection = new Closing();

  /**
   * Shows a choice with `show`, which gets the signal that closes it, and
   * gives what it answers. A request canceled before it opens, of which the
   * token will tell nothing more, opens nothing and answers `null`.
   */
  async open<T>(
    canceled: Cancellation,
    show: (closing: Closing) => PromiseLike<T | null>,
  ): Promise<T | null> {
    if (canceled.isCancellationRequested) {
      return null;
    }

    const closing = new Closing();
    const subscription = canceled.onCancellationRequested(() => closing.close());
    const unlink = this.connection.onClose(() => closing.close());
    try {
      return await show(closing);
    } finally {
      subscription.dispose();
      unlink();
    }
  }

  /** Closes every choice of the connection that ended, and gives the next connection its own. */
  endConnection(): void {
    const ended = this.connection;
    this.connection = new Closing();
    ended.close();
  }
}
