// The server process owned by one language client, including its automatic restarts.

import { type ChildProcess, spawn } from "node:child_process";
import { once } from "node:events";

/** The language client's shutdown deadline, also allowed for exit after shutdown. */
export const STOP_TIMEOUT_MS = 2000;

export interface ProcessOwner {
  process?: ChildProcess;
  retired: boolean;
}

/** Starts the owner's next process and reports OS errors to the language client. */
export async function launch(
  owner: ProcessOwner,
  command: string,
  cwd: string | undefined,
): Promise<ChildProcess> {
  if (owner.retired) {
    throw new Error("The language server has been stopped");
  }
  const child = spawn(command, [], { cwd });
  owner.process = child;
  // Errors after the start, such as of a kill, show in how the process ends.
  child.on("error", () => {});
  await once(child, "spawn");
  if (owner.retired) {
    kill(child);
    throw new Error("The language server has been stopped");
  }
  return child;
}

/** Ends a disconnected child unless its owner is already managing shutdown. */
export function killDisconnected(owner: ProcessOwner): void {
  if (!owner.retired && owner.process) {
    kill(owner.process);
  }
}

/** Prevents further launches and ends the current process, allowing a graceful shutdown when available. */
export async function retire(owner: ProcessOwner, shutdown?: () => Promise<void>): Promise<void> {
  owner.retired = true;
  const child = owner.process;
  try {
    if (shutdown) {
      await shutdown();
      if (child) {
        await waitForExit(child, STOP_TIMEOUT_MS);
      }
    }
  } finally {
    if (child) {
      kill(child);
    }
  }
}

/** Waits until `child` exits, or until `timeoutMs` passes. */
function waitForExit(child: ChildProcess, timeoutMs: number): Promise<void> {
  if (!isRunning(child)) {
    return Promise.resolve();
  }
  return new Promise<void>((resolve) => {
    const finish = () => {
      clearTimeout(timer);
      child.off("exit", finish);
      resolve();
    };
    const timer = setTimeout(finish, timeoutMs);
    child.once("exit", finish);
  });
}

function isRunning(child: ChildProcess): boolean {
  return child.exitCode === null && child.signalCode === null;
}

function kill(child: ChildProcess): void {
  if (isRunning(child)) {
    child.kill("SIGKILL");
  }
}
