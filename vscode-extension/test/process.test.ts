import { once } from "node:events";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { expect, onTestFinished, test } from "vitest";
import {
  type ProcessOwner,
  STOP_TIMEOUT_MS,
  killDisconnected,
  launch,
  retire,
} from "../src/process";

test(
  "a file that may not be run fails to launch with the error of the system",
  { skip: process.platform === "win32" },
  async () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "translation-lsp-launch-"));
    const owner = { retired: false };
    try {
      const file = path.join(directory, "translation-lsp");
      fs.writeFileSync(file, "", { mode: 0o600 });
      await expect(launch(owner, file, undefined)).rejects.toMatchObject({ code: "EACCES" });
      await expect(launch(owner, path.join(directory, "missing"), undefined)).rejects.toMatchObject(
        { code: "ENOENT" },
      );
    } finally {
      fs.rmSync(directory, { recursive: true });
    }
  },
);

test("retiring before the language client requests a process prevents the launch", async () => {
  const owner: ProcessOwner = { retired: false };
  onTestFinished(() => {
    owner.process?.kill("SIGKILL");
  });

  await retire(owner);
  await expect(launch(owner, process.execPath, undefined)).rejects.toThrow(/stopped/);
  expect(owner.process).toBe(undefined);
});

for (const attempt of ["first launch", "automatic restart"]) {
  test(
    `retiring during the ${attempt} ends the child and rejects the launch`,
    { timeout: 5000 },
    async () => {
      const owner: ProcessOwner = { retired: false };
      if (attempt === "automatic restart") {
        const first = await launch(owner, process.execPath, undefined);
        const exit = once(first, "exit");
        first.stdin?.end();
        await exit;
      }

      const pending = launch(owner, process.execPath, undefined);
      onTestFinished(async () => {
        const child = await pending.catch(() => owner.process);
        child?.kill("SIGKILL");
      });
      const child = owner.process;
      expect.assert.ok(child?.pid);

      const exit = once(child, "exit");
      await retire(owner);
      await expect(pending).rejects.toThrow(/stopped/);
      await exit;
      expect(child.exitCode !== null || child.signalCode !== null).toBe(true);
    },
  );
}

test(
  "a disconnect ends the child without waiting for a restart or retirement",
  { timeout: 5000 },
  async ({ signal }) => {
    const owner = { retired: false };
    const first = await launch(owner, process.execPath, undefined);
    onTestFinished(() => {
      first.kill("SIGKILL");
    });
    expect.assert.ok(first.stdout);

    const disconnected = once(first.stdout, "end");
    first.stdout.resume();
    first.stdin?.end("process.stdout.end(); setInterval(() => {}, 1000);");
    await disconnected;
    expect(first.exitCode).toBe(null);
    expect(first.signalCode).toBe(null);

    const firstExit = once(first, "exit", { signal });
    killDisconnected(owner);
    await firstExit;

    const next = await launch(owner, process.execPath, undefined);
    onTestFinished(() => {
      next.kill("SIGKILL");
    });
    const nextExit = once(next, "exit");
    await retire(owner);
    await nextExit;
  },
);

test("retiring ends a child even when it ignores SIGTERM", { timeout: 5000 }, async () => {
  const owner = { retired: false };
  const child = await launch(owner, process.execPath, undefined);
  onTestFinished(() => {
    child.kill("SIGKILL");
  });
  const exit = once(child, "exit");

  if (process.platform !== "win32") {
    // The acknowledgment proves the child handled SIGTERM and kept running.
    child.stdin?.end(
      'process.on("SIGTERM", () => console.log("handled")); setInterval(() => {}, 1000); console.log("ready");',
    );
    expect.assert.ok(child.stdout);
    await once(child.stdout, "data");
    child.kill("SIGTERM");
    const [reply] = await once(child.stdout, "data");
    expect(reply.toString().trim()).toBe("handled");
  }

  await retire(owner);
  await exit;
  await retire(owner);
});

test("a child can finish cleanup after shutdown and exit normally", { timeout: 5000 }, async () => {
  const owner = { retired: false };
  const child = await launch(owner, process.execPath, undefined);
  onTestFinished(() => {
    child.kill("SIGKILL");
  });

  await retire(owner, async () => {
    child.stdin?.end("setTimeout(() => process.exit(0), 50);");
    killDisconnected(owner);
  });
  expect(child.exitCode).toBe(0);
  expect(child.signalCode).toBe(null);
});

test(
  "retirement gives a child a bounded grace period after shutdown",
  { timeout: 6000 },
  async () => {
    const owner = { retired: false };
    // Node keeps waiting for its standard input even after shutdown returns.
    const child = await launch(owner, process.execPath, undefined);
    onTestFinished(() => {
      child.kill("SIGKILL");
    });

    const exit = once(child, "exit");
    const started = Date.now();
    await retire(owner, async () => {});
    await exit;
    // Allow for timer rounding.
    expect(Date.now() - started).toBeGreaterThanOrEqual(STOP_TIMEOUT_MS - 10);
    expect(child.exitCode !== null || child.signalCode !== null).toBe(true);
  },
);
