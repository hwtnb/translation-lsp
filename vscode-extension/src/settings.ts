// The server settings that the VS Code settings of this extension stand for.
// Nothing here uses the VS Code API, so the tests run it without VS Code.

import * as fs from "node:fs";
import * as path from "node:path";
import manifest from "../package.json";
import providerFields from "./providers.json";

/** The section of the VS Code settings of this extension. */
export const SECTION = "translationLsp";

/** The VS Code setting that names the server executable, which the server never sees. */
export const SERVER_PATH = "server.path";

/** The settings that each provider takes, named as the server names them inside `provider`. */
export const PROVIDER_FIELDS: Readonly<Record<string, readonly string[]>> = providerFields;

/** Reads the value that the user wrote for a setting, named without the section, or `undefined`. */
export type Read = (name: string) => unknown;

/** What VS Code tells of one setting: the values written where they count, and the value they make together. */
export interface Inspected {
  globalValue?: unknown;
  workspaceValue?: unknown;
  value: unknown;
}

/**
 * The value of a setting that the user wrote in the user or the workspace
 * settings, as VS Code merges them, or `undefined` if they wrote it in
 * neither. The server serves the whole window, so values of a workspace
 * folder do not count, and these settings have no values for a language.
 */
export function pickUserValue({ globalValue, workspaceValue, value }: Inspected): unknown {
  return globalValue === undefined && workspaceValue === undefined ? undefined : value;
}

/** Why the server cannot start from `serverPath`, as the settings give it, or `undefined` if it can. */
export function checkServerPath(
  serverPath: unknown,
): "not text" | "empty" | "relative" | "missing" | "not a file" | "unreadable" | undefined {
  if (typeof serverPath !== "string") {
    return "not text";
  }
  if (!serverPath) {
    return "empty";
  }
  if (!path.isAbsolute(serverPath)) {
    return "relative";
  }
  let stat: fs.Stats | undefined;
  try {
    stat = fs.statSync(serverPath, { throwIfNoEntry: false });
  } catch {
    // Such as a folder that may not be read, or a loop of links.
    return "unreadable";
  }
  if (!stat) {
    return "missing";
  }
  return stat.isFile() ? undefined : "not a file";
}

/** Why the server cannot start, as `checkServerPath` tells it. */
export type ServerProblem = NonNullable<ReturnType<typeof checkServerPath>>;

/**
 * The server to start: the one the setting names whenever it holds a value,
 * so that a path the user wrote is never passed over, or else the one in the
 * `server` folder of the extension. `problem` tells why neither starts, with
 * `empty` for an extension that holds no server.
 */
export function locateServer(
  setting: unknown,
  extensionPath: string,
  platform: NodeJS.Platform = process.platform,
): { path: string } | { problem: ServerProblem } {
  if (setting !== "") {
    const problem = checkServerPath(setting);
    return problem ? { problem } : { path: setting as string };
  }
  const bundled = path.join(extensionPath, "server", nameServerExecutable(platform));
  return checkServerPath(bundled) ? { problem: "empty" } : { path: bundled };
}

/** The file name of the server executable on `platform`. */
export function nameServerExecutable(platform: NodeJS.Platform): string {
  return platform === "win32" ? "translation-lsp.exe" : "translation-lsp";
}

/** The name of a server setting as VS Code names it: each part in camelCase, joined with dots. */
export function toVscodeName(serverName: string): string {
  return serverName.replace(/_([a-z0-9])/g, (_, letter: string) => letter.toUpperCase());
}

/** The name of a server setting that a VS Code setting stands for. */
export function toServerName(vscodeName: string): string {
  return vscodeName.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

/** The VS Code settings that stand for server settings, named without the section. */
export function listServerSettings(): string[] {
  return manifest.contributes.configuration
    .flatMap((group) => Object.keys(group.properties))
    .map((name) => name.slice(SECTION.length + 1))
    .filter((name) => name !== SERVER_PATH);
}

/**
 * The `initializationOptions` of the server: the settings the user wrote, so
 * that the server decides every default. Of the provider settings, only those
 * of the chosen provider are sent, and those of `provider.document` only with
 * its model.
 */
export function buildInitializationOptions(
  read: Read,
  names = listServerSettings(),
): Record<string, unknown> {
  const options: Record<string, unknown> = {};
  for (const name of names.filter((name) => !name.startsWith("provider."))) {
    const value = toSent(name, read(name));
    if (value !== undefined) {
      place(options, toServerName(name), value);
    }
  }

  const provider = toSent("provider.name", read("provider.name"));
  if (typeof provider !== "string") {
    return options;
  }

  const settings: Record<string, unknown> = { name: provider };
  const document = toSent("provider.document.model", read("provider.document.model")) !== undefined;
  // Only the names of the table: `constructor` names no row of it.
  const fields = Object.hasOwn(PROVIDER_FIELDS, provider) ? PROVIDER_FIELDS[provider] : [];
  for (const field of fields) {
    if (field.startsWith("document.") && !document) {
      continue;
    }
    const name = toVscodeName(`provider.${field}`);
    if (!names.includes(name)) {
      continue;
    }
    const value = toSent(name, read(name));
    if (value !== undefined) {
      place(settings, field, value);
    }
  }

  options.provider = settings;
  return options;
}

/**
 * The value to send for `value` of setting `name`, or `undefined` for none:
 * an empty text is not written, and `auto` of a thinking setting leaves the
 * choice to the server.
 */
function toSent(name: string, value: unknown): unknown {
  if (value === undefined || value === null || value === "") {
    return undefined;
  }
  if (name.endsWith(".thinking")) {
    return value === "on" ? true : value === "off" ? false : undefined;
  }
  return value;
}

/** Puts `value` into `object` at the dotted `path`. */
function place(object: Record<string, unknown>, path: string, value: unknown): void {
  const parts = path.split(".");
  const last = parts.pop()!;
  let inner = object;
  for (const part of parts) {
    const next = inner[part];
    inner =
      typeof next === "object" && next !== null
        ? (next as Record<string, unknown>)
        : (inner[part] = {});
  }
  inner[last] = value;
}
