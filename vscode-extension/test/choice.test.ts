import { expect, test } from "vitest";
import {
  type Closing,
  OpenChoices,
  answerWith,
  awaitUnlessClosed,
  readChoiceRequest,
} from "../src/choice";

const CHOICES = [
  { id: "0", title: "Turn off automatic translation" },
  { id: "1", title: "Anthropic API key…" },
];

test("a request to choose is read only in its own shape", () => {
  const menu = { kind: "menu", message: "Automatic translation: On", choices: CHOICES };
  expect(readChoiceRequest(menu)).toStrictEqual(menu);
  const confirm = { kind: "confirm", message: "Delete?", choices: [{ id: "0", title: "Delete" }] };
  expect(readChoiceRequest(confirm)).toStrictEqual(confirm);
  // Fields that the request does not define are left out.
  expect(
    readChoiceRequest({ ...menu, extra: true, choices: [{ ...CHOICES[0], extra: 1 }] }),
  ).toStrictEqual({ ...menu, choices: [CHOICES[0]] });

  for (const params of [
    null,
    "menu",
    [],
    { ...menu, kind: "notice" },
    { ...menu, message: undefined },
    { ...menu, message: 1 },
    { ...menu, choices: undefined },
    { ...menu, choices: {} },
    { ...menu, choices: [{ id: 0, title: "Zero" }] },
    { ...menu, choices: [{ id: "0" }] },
    { ...menu, choices: [null] },
  ]) {
    expect(readChoiceRequest(params), JSON.stringify(params)).toBe(undefined);
  }
});

test("the answer names the choice picked by its ID, or nothing", () => {
  expect(answerWith(CHOICES[1])).toStrictEqual({ id: "1" });
  expect(answerWith(undefined)).toBe(null);
});

test("a part that should have closed answers nothing", async () => {
  let close!: () => void;
  const closed = new Promise<void>((resolve) => {
    close = resolve;
  });
  const never = new Promise<string>(() => {});
  const waiting = awaitUnlessClosed(never, closed);
  close();
  await expect(waiting).resolves.toBe(undefined);
  await expect(awaitUnlessClosed(Promise.resolve("picked"), new Promise(() => {}))).resolves.toBe(
    "picked",
  );
});

/** A request token that the test cancels, as the library gives it. */
function makeToken() {
  let listeners: (() => void)[] = [];
  return {
    isCancellationRequested: false,
    onCancellationRequested(listener: () => void) {
      listeners.push(listener);
      return {
        dispose: () => {
          listeners = listeners.filter((other) => other !== listener);
        },
      };
    },
    cancel() {
      this.isCancellationRequested = true;
      for (const listener of listeners) {
        listener();
      }
    },
  };
}

/** A Quick Pick that stays open until the user picks or `closing` closes it. */
function makePicker() {
  const opened: { closing: Closing; pick: (answer: { id: string }) => void }[] = [];
  const show = (closing: Closing) =>
    new Promise<{ id: string } | null>((resolve) => {
      closing.onClose(() => resolve(null));
      opened.push({ closing, pick: resolve });
    });
  return { opened, show };
}

test("a request canceled before it opens shows nothing", async () => {
  const choices = new OpenChoices();
  const { opened, show } = makePicker();
  // The library gives such a request a token that tells nothing more.
  const token = { ...makeToken(), isCancellationRequested: true };
  await expect(choices.open(token, show)).resolves.toBe(null);
  expect(opened).toHaveLength(0);
});

test("a request canceled after it opens closes its choice", async () => {
  const choices = new OpenChoices();
  const { opened, show } = makePicker();
  const token = makeToken();
  const answer = choices.open(token, show);
  expect(opened).toHaveLength(1);

  token.cancel();
  expect(opened[0].closing.isClosed).toBe(true);
  await expect(answer).resolves.toBe(null);
});

test("the end of a connection closes its choices, and the next connection opens its own", async () => {
  const choices = new OpenChoices();
  const { opened, show } = makePicker();
  const before = choices.open(makeToken(), show);
  const other = choices.open(makeToken(), show);

  // A restart after the connection broke.
  choices.endConnection();
  await expect(before).resolves.toBe(null);
  await expect(other).resolves.toBe(null);

  const after = choices.open(makeToken(), show);
  expect(opened).toHaveLength(3);
  expect(opened[2].closing.isClosed).toBe(false);
  opened[2].pick({ id: "1" });
  await expect(after).resolves.toStrictEqual({ id: "1" });
});
