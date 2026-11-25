import * as fs from "node:fs";
import * as vscode from "vscode";
import {
  CloseAction,
  ErrorAction,
  type ErrorHandler,
  ExecuteCommandRequest,
  LanguageClient,
  type LanguageClientOptions,
  RevealOutputChannelOn,
  State,
  type StaticFeature,
} from "vscode-languageclient/node";
import { type Answer, CHOOSE, OpenChoices, readChoiceRequest } from "./choice";
import { showChoice } from "./choose";
import { Lifecycle } from "./lifecycle";
import { type ProcessOwner, STOP_TIMEOUT_MS, killDisconnected, launch, retire } from "./process";
import {
  SECTION,
  SERVER_PATH,
  buildInitializationOptions,
  locateServer,
  pickUserValue,
} from "./settings";
import { SHOW_TRANSLATION, type Shown, TranslationThreads, tellUntold } from "./translation";

/** The request with which the server asks for the settings of this extension. */
const OPEN_SETTINGS = "translation-lsp/openSettings";
/** What the Settings editor is searched for to show the settings of this extension. */
const OWN_SETTINGS = "@ext:hwtnb.translation-lsp";

/** A client with the server process that the extension started for it. */
interface Server extends ProcessOwner {
  client: LanguageClient;
  /** The menus and confirmations open on the connection to the server. */
  choices: OpenChoices;
}

let lifecycle: Lifecycle<Server> | undefined;
let threads: TranslationThreads | undefined;
/**
 * The mark of the notice that settings changed, while it is shown and no
 * restart came after it, so that it is not shown twice.
 */
let openNotice: number | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const started = new Lifecycle<Server>({ start: () => start(context), stop: stopServer });
  lifecycle = started;

  threads = new TranslationThreads((command, args) => {
    const client = started.client?.client;
    if (!client) {
      return Promise.reject(new Error(vscode.l10n.t("The language server is not running")));
    }
    return client.sendRequest(ExecuteCommandRequest.type, { command, arguments: args });
  });

  context.subscriptions.push(
    threads,
    vscode.commands.registerCommand("translationLsp.restartServer", () => started.restart()),
    vscode.commands.registerCommand("translationLsp.copyTranslation", () => threads?.copy()),
    vscode.commands.registerCommand("translationLsp.retryTranslation", () => threads?.act("retry")),
    vscode.commands.registerCommand("translationLsp.deleteTranslation", () =>
      threads?.act("delete"),
    ),
    vscode.commands.registerCommand("translationLsp.moreActions", () => threads?.act(undefined)),
    vscode.commands.registerCommand("translationLsp.closeTranslation", () => threads?.close()),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration(SECTION)) {
        void announceChange(started);
      }
    }),
  );

  await started.restart();
}

export async function deactivate(): Promise<void> {
  await lifecycle?.end();
}

/**
 * Tells that changed settings apply after a restart, and restarts when asked,
 * unless a restart came after the notice or the end has begun.
 */
async function announceChange(lifecycle: Lifecycle<Server>): Promise<void> {
  const mark = lifecycle.mark();
  if (openNotice === mark) {
    return;
  }

  openNotice = mark;
  const restartButton = vscode.l10n.t("Restart");
  const choice = await vscode.window.showInformationMessage(
    vscode.l10n.t("Restart the language server to apply the changed Translation LSP settings."),
    restartButton,
  );

  if (openNotice === mark) {
    openNotice = undefined;
  }
  if (choice === restartButton) {
    await lifecycle.restartFrom(mark);
  }
}

/** Stops a running client gracefully and interrupts one that is still starting. */
function stopServer(server: Server): Promise<void> {
  threads?.close();
  server.choices.endConnection();
  return retire(
    server,
    server.client.state === State.Running ? () => server.client.stop(STOP_TIMEOUT_MS) : undefined,
  );
}

/** The folder that the server starts in, as the library chooses it: the first folder of the workspace. */
function findServerFolder(): string | undefined {
  const folder = vscode.workspace.workspaceFolders?.[0]?.uri;
  if (folder?.scheme !== "file") {
    return undefined;
  }
  try {
    return fs.statSync(folder.fsPath).isDirectory() ? folder.fsPath : undefined;
  } catch {
    return undefined;
  }
}

function openSettings(query = OWN_SETTINGS): Thenable<unknown> {
  return vscode.commands.executeCommand("workbench.action.openSettings", query);
}

/** The value that the user wrote for `name`, named without the section. */
function readWritten(name: string): unknown {
  const settings = vscode.workspace.getConfiguration(SECTION);
  return pickUserValue({ ...settings.inspect(name), value: settings.get(name) });
}

/** Tells that the server cannot start, with a button that opens the setting of its path. */
function reportServerPath(message: string): undefined {
  const settingsButton = vscode.l10n.t("Open Settings");
  void vscode.window.showErrorMessage(message, settingsButton).then((choice) => {
    if (choice === settingsButton) {
      void openSettings(`${SECTION}.${SERVER_PATH}`);
    }
  });
  return undefined;
}

/**
 * Starts a client with the settings as they are now, without waiting for its
 * server to answer, or tells why none starts.
 */
async function start(context: vscode.ExtensionContext): Promise<Server | undefined> {
  const command = readServerPath(context.extensionPath);
  if (command === undefined) {
    return undefined;
  }

  // The options and the launch read `server` only once the client runs.
  const client = new LanguageClient(
    "translation-lsp",
    "Translation LSP",
    () => launch(server, command, findServerFolder()),
    buildClientOptions(context, () => server),
  );
  const server: Server = { client, retired: false, choices: new OpenChoices() };
  registerHandlers(server);
  client.start().catch(() => {
    // The library, or the initialization failure handler, showed why.
  });
  return server;
}

/**
 * The path of the server to start, from the settings or the extension, or
 * `undefined` after telling why none starts.
 */
function readServerPath(extensionPath: string): string | undefined {
  const path = vscode.workspace.getConfiguration(SECTION).get<unknown>(SERVER_PATH, "");
  const server = locateServer(path, extensionPath);
  if ("path" in server) {
    return server.path;
  }
  switch (server.problem) {
    case "not text":
      return reportServerPath(
        vscode.l10n.t(
          "The path of the Translation LSP server must be text: {0}",
          JSON.stringify(path),
        ),
      );
    case "empty":
      return reportServerPath(
        vscode.l10n.t(
          "This copy of the extension holds no Translation LSP server. Set the path of a server that you built to start it.",
        ),
      );
    case "relative":
      return reportServerPath(
        vscode.l10n.t("The path of the Translation LSP server must be absolute: {0}", String(path)),
      );
    case "missing":
      return reportServerPath(
        vscode.l10n.t("The Translation LSP server is not found at {0}.", String(path)),
      );
    case "not a file":
      return reportServerPath(
        vscode.l10n.t("The path of the Translation LSP server is not a file: {0}", String(path)),
      );
    case "unreadable":
      return reportServerPath(
        vscode.l10n.t("The path of the Translation LSP server cannot be read: {0}", String(path)),
      );
  }
}

/** The options of the client of `server`, which they read only once the client runs. */
function buildClientOptions(
  context: vscode.ExtensionContext,
  getServer: () => Server,
): LanguageClientOptions {
  const languages: string[] = context.extension.packageJSON.activationEvents
    .filter((event: string) => event.startsWith("onLanguage:"))
    .map((event: string) => event.slice("onLanguage:".length));
  let defaultHandler: ErrorHandler | undefined;
  const getDefaultHandler = () =>
    (defaultHandler ??= getServer().client.createDefaultErrorHandler());

  return {
    documentSelector: languages.flatMap((language) => [
      { scheme: "file", language },
      { scheme: "untitled", language },
    ]),
    initializationOptions: buildInitializationOptions(readWritten),
    initializationFailedHandler: (error) => {
      // A server that the extension stopped while it started has nothing to tell.
      if (!getServer().retired) {
        reportInitializationFailure(error);
      }
      return false;
    },
    // The server tells of the failures that the user needs to know itself.
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    // A server that the extension stops is not started again.
    errorHandler: {
      error: (error, message, count) =>
        getServer().retired
          ? { action: ErrorAction.Shutdown, handled: true }
          : getDefaultHandler().error(error, message, count),
      closed: () => {
        getServer().choices.endConnection();
        killDisconnected(getServer());
        return getServer().retired
          ? { action: CloseAction.DoNotRestart, handled: true }
          : getDefaultHandler().closed();
      },
    },
    traceOutputChannel: noTrace,
    markdown: { isTrusted: false, supportHtml: false, supportThemeIcons: true },
    middleware: {
      // The server tells of the failures of its commands itself; VS Code
      // would tell of a CodeLens command that fails again.
      async executeCommand(command, args, next) {
        try {
          return await next(command, args);
        } catch (error) {
          if (error instanceof vscode.CancellationError) {
            throw error;
          }
          tellUntold(error);
          return undefined;
        }
      },
    },
  };
}

/** Tells that the server failed to initialize, with a button that opens the settings. */
function reportInitializationFailure(error: Error): void {
  const settingsButton = vscode.l10n.t("Open Settings");
  void vscode.window.showErrorMessage(error.message, settingsButton).then((choice) => {
    if (choice === settingsButton) {
      void openSettings();
    }
  });
}

/** Takes the messages of the server of `server` that this extension handles. */
function registerHandlers(server: Server): void {
  const { client } = server;
  client.registerFeature(naming);
  client.onNotification(SHOW_TRANSLATION, (shown: Shown) => {
    if (lifecycle?.client === server) {
      threads?.show(shown);
    }
  });
  client.onRequest(OPEN_SETTINGS, async () => {
    await openSettings();
    return null;
  });
  client.onRequest(CHOOSE, (params: unknown, token: vscode.CancellationToken) =>
    choose(server, params, token),
  );
  // A translation of a server that stopped, or of one that restarted after
  // it ended, carries commands that its successor may take differently, and
  // the requests of its choices end with its connection.
  client.onDidChangeState((event) => {
    if (event.newState !== State.Running) {
      threads?.close();
      server.choices.endConnection();
    }
  });
}

/**
 * Shows a menu or confirmation that the server asks for, until the user
 * answers, the server withdraws it, or its connection ends. A request of
 * another shape, or one to a server that the extension stops, picks nothing.
 */
function choose(server: Server, params: unknown, token: vscode.CancellationToken): Promise<Answer> {
  const request = readChoiceRequest(params);
  if (!request || server.retired) {
    return Promise.resolve(null);
  }
  return server.choices.open(token, (closing) => showChoice(request, closing));
}

/** Names the messages of the server that this extension takes, under `experimental.translationLsp`. */
const naming: StaticFeature = {
  fillClientCapabilities(capabilities) {
    capabilities.experimental = {
      ...(capabilities.experimental as object | undefined),
      translationLsp: { showTranslation: true, openSettings: true, choose: true, themeIcons: true },
    };
  },
  initialize() {},
  getState() {
    return { kind: "static" };
  },
  clear() {},
};

/**
 * The channel of the message trace, which writes nothing and never asks for a
 * trace: a trace would hold the text of documents and the settings.
 */
const noTrace: vscode.LogOutputChannel = {
  name: "Translation LSP Trace",
  logLevel: vscode.LogLevel.Off,
  onDidChangeLogLevel: new vscode.EventEmitter<vscode.LogLevel>().event,
  trace() {},
  debug() {},
  info() {},
  warn() {},
  error() {},
  append() {},
  appendLine() {},
  replace() {},
  clear() {},
  show() {},
  hide() {},
  dispose() {},
};
