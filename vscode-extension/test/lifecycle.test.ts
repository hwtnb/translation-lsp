import { expect, test } from "vitest";
import { type Hooks, Lifecycle } from "../src/lifecycle";

/** A promise with the functions that settle it. */
function defer(): { promise: Promise<void>; resolve: () => void; reject: (error: Error) => void } {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

/**
 * A client of the tests: the settings it started with, whether its server
 * runs, and whether that server ever answered.
 */
interface Client {
  settings: string;
  running: boolean;
  answered: boolean;
}

/**
 * Hooks that start clients with `settings.value`, whose servers answer only
 * if `answers`, and that wait for a gate, if set, before a start ends or a
 * stop ends its server, and fail the stop if `stopFails`.
 */
function makeHooks() {
  const record = {
    settings: { value: "first" },
    started: [] as Client[],
    answers: true,
    startGate: undefined as ReturnType<typeof defer> | undefined,
    stopGate: undefined as ReturnType<typeof defer> | undefined,
    stopFails: false,
  };
  const hooks: Hooks<Client> = {
    async start() {
      const client = { settings: record.settings.value, running: true, answered: record.answers };
      record.started.push(client);
      await record.startGate?.promise;
      return client;
    },
    async stop(client) {
      try {
        await record.stopGate?.promise;
        if (record.stopFails) {
          throw new Error("Stopping the server timed out");
        }
      } finally {
        client.running = false;
      }
    },
  };
  return { record, hooks };
}

test("restarts asked for during a cycle run once more after it", async () => {
  const { record, hooks: started } = makeHooks();
  const lifecycle = new Lifecycle(started);
  await lifecycle.restart();
  record.stopGate = defer();
  const restarts = [lifecycle.restart(), lifecycle.restart(), lifecycle.restart()];
  record.stopGate.resolve();
  await Promise.all(restarts);
  // The first start, the cycle, and one more for the two that came during it.
  expect(record.started.length).toBe(3);
  expect(record.started.map((client) => client.running)).toStrictEqual([false, false, true]);
  expect(lifecycle.client).toBe(record.started[2]);
});

test("settings changed after a start read them apply in the cycle after it", async () => {
  const { record, hooks: started } = makeHooks();
  const lifecycle = new Lifecycle(started);
  record.startGate = defer();
  const first = lifecycle.restart();
  // The start has read the settings and launches the server.
  record.settings.value = "second";
  const pressed = lifecycle.restartFrom(lifecycle.mark());
  record.startGate.resolve();
  await Promise.all([first, pressed]);
  expect(record.started.map((client) => [client.settings, client.running])).toStrictEqual([
    ["first", false],
    ["second", true],
  ]);
});

test("a restart while a server never answers ends it and starts with the new settings", async () => {
  const { record, hooks: started } = makeHooks();
  const lifecycle = new Lifecycle(started);
  record.answers = false;
  // The start returns its client without waiting for an answer.
  await lifecycle.restart();
  const silent = lifecycle.client;
  expect(silent?.answered).toBe(false);

  record.answers = true;
  record.settings.value = "fixed";
  await lifecycle.restart();
  expect(silent?.running).toBe(false);
  expect(lifecycle.client).toStrictEqual({ settings: "fixed", running: true, answered: true });

  // The end does not wait for an answer either.
  record.answers = false;
  await lifecycle.restart();
  await lifecycle.end();
  expect(record.started.every((client) => !client.running)).toBe(true);
});

test("a stop that fails ends the server, and a restart asked for meanwhile still runs", async () => {
  const { record, hooks: started } = makeHooks();
  const lifecycle = new Lifecycle(started);
  await lifecycle.restart();

  record.stopGate = defer();
  record.stopFails = true;
  const first = lifecycle.restart();
  const second = lifecycle.restart();
  record.stopGate.resolve();
  record.stopGate = undefined;
  await Promise.all([first, second]);

  // The failed stop, the cycle's start, and one more for the second restart.
  expect(record.started.length).toBe(3);
  expect(record.started.map((client) => client.running)).toStrictEqual([false, false, true]);
});

test("a notice made stale by a later restart restarts nothing", async () => {
  const { record, hooks: started } = makeHooks();
  const lifecycle = new Lifecycle(started);
  await lifecycle.restart();
  const notice = lifecycle.mark();
  // The command restarts first.
  await lifecycle.restart();
  await lifecycle.restartFrom(notice);
  expect(record.started.length).toBe(2);
  // A notice shown after it still works.
  await lifecycle.restartFrom(lifecycle.mark());
  expect(record.started.length).toBe(3);
});

test("no restart runs once the end has begun", async () => {
  const { record, hooks: started } = makeHooks();
  const lifecycle = new Lifecycle(started);
  await lifecycle.restart();
  const notice = lifecycle.mark();
  record.stopGate = defer();
  const ending = lifecycle.end();
  await lifecycle.restartFrom(notice);
  await lifecycle.restart();
  record.stopGate.resolve();
  await ending;
  expect(record.started.length).toBe(1);
  expect(record.started[0].running).toBe(false);
  expect(lifecycle.client).toBe(undefined);
});

test("the end waits for the cycle in progress and stops what it started", async () => {
  const { record, hooks: started } = makeHooks();
  const lifecycle = new Lifecycle(started);
  record.startGate = defer();
  const first = lifecycle.restart();
  const ending = lifecycle.end();
  record.startGate.resolve();
  await Promise.all([first, ending]);
  expect(record.started.length).toBe(1);
  expect(record.started[0].running).toBe(false);
  expect(lifecycle.client).toBe(undefined);
});
