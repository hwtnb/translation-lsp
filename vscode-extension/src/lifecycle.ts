// When the client of the server starts and stops. Nothing here uses the VS
// Code API, so the tests run it without VS Code.

/** How the lifecycle starts and stops a client. */
export interface Hooks<C> {
  /**
   * Starts a client with the settings as they are now, without waiting for
   * its server to answer, or gives none if it cannot start.
   */
  start(): Promise<C | undefined>;
  /**
   * Stops `client`, and ends its server in any case, whether it answered,
   * still starts, or fails to stop.
   */
  stop(client: C): Promise<void>;
}

/**
 * The client of the server, which one cycle at a time stops and starts again.
 * A cycle never waits for a server to answer, so a server that never does
 * keeps no restart from running. A restart asked for during a cycle runs once
 * more after it, with the settings as they are then, however many were asked
 * for. Once the end has begun, no restart runs.
 */
export class Lifecycle<C> {
  private current?: C;
  private cycle?: Promise<void>;
  /** Whether a restart was asked for during the cycle. */
  private again = false;
  /** How many restarts were asked for, which tells a stale mark. */
  private restarts = 0;
  private ending = false;

  constructor(private readonly hooks: Hooks<C>) {}

  /** The client, from its start until it is stopped. */
  get client(): C | undefined {
    return this.current;
  }

  /** Stops the client, if there is one, and starts one with the settings as they are now. */
  restart(): Promise<void> {
    if (this.ending) {
      return this.cycle ?? Promise.resolve();
    }
    this.restarts += 1;
    if (this.cycle) {
      this.again = true;
      return this.cycle;
    }
    this.cycle = this.run().finally(() => {
      this.cycle = undefined;
    });
    return this.cycle;
  }

  /** A mark of the restarts so far, for a request to restart that a later restart makes stale. */
  mark(): number {
    return this.restarts;
  }

  /** Restarts, unless a restart came after `mark` or the end has begun. */
  restartFrom(mark: number): Promise<void> {
    return mark === this.restarts && !this.ending ? this.restart() : Promise.resolve();
  }

  /** Stops the client for good, after the cycle in progress. */
  async end(): Promise<void> {
    this.ending = true;
    await this.cycle;
    await this.stopCurrent();
  }

  private async run(): Promise<void> {
    do {
      this.again = false;
      if (this.current !== undefined) {
        await this.stopCurrent();
      }
      if (this.ending) {
        return;
      }
      this.current = await this.hooks.start();
    } while (this.again && !this.ending);
  }

  private async stopCurrent(): Promise<void> {
    const client = this.current;
    this.current = undefined;
    if (client !== undefined) {
      try {
        await this.hooks.stop(client);
      } catch {
        // The hook ended the server whatever failed.
      }
    }
  }
}
