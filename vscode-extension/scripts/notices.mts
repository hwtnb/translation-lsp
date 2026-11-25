// Writes the licenses of the npm packages that `out/extension.js` holds code
// of to THIRD_PARTY_LICENSES.txt: those that the build lists in its source
// map, and Rolldown, whose helpers the build adds without listing them.

import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

/** The licenses that a bundled package may declare. Review any other before adding it. */
export const ACCEPTED = ["MIT", "ISC", "BlueOak-1.0.0"];

/** The files of a package that hold its license or its notices, by name without case. */
const LICENSE_FILE = /^(licen[cs]e|copying)(\.(md|txt))?$/i;
const NOTICE_FILE = /^third-?party-?(notices|licen[cs]es?)(\.(md|txt))?$/i;

/** The bundler, whose runtime helpers start `out/extension.js` but are not in its source map. */
const BUNDLER = "node_modules/rolldown";

/** What the notices tell of a bundled package. */
export interface BundledPackage {
  name: string;
  version: string;
  license: string;
  /** Where the lockfile says the package was downloaded from. */
  source: string;
  /** The license files and notices of the package, by file name. */
  files: { name: string; text: string }[];
}

/**
 * The folders of the packages, relative to `root` and in the order of
 * `sources`, that the sources of a source map in `mapFolder` belong to.
 * Sources of the extension itself, in `src` and its `package.json`, belong to
 * none; any other source outside `node_modules` cannot be told and fails.
 */
export function listPackageFolders(sources: string[], mapFolder: string, root: string): string[] {
  const folders = new Set<string>();
  for (const source of sources) {
    const relative = path.relative(root, path.resolve(mapFolder, source)).split(path.sep);
    if (relative[0] === ".." || path.isAbsolute(relative.join(path.sep))) {
      throw new Error(`The bundle holds code from outside the extension: ${source}`);
    }
    const last = relative.lastIndexOf("node_modules");
    if (last === -1) {
      if (relative[0] !== "src" && relative.join("/") !== "package.json") {
        throw new Error(`The bundle holds code of neither the extension nor a package: ${source}`);
      }
      continue;
    }
    const length = relative[last + 1]?.startsWith("@") ? 3 : 2;
    folders.add(relative.slice(0, last + length).join("/"));
  }
  return [...folders];
}

/**
 * Reads the package in `folder` of `root`, with its download address from
 * `lockfile`. Fails for a package that declares a license outside `ACCEPTED`,
 * and for one without a license file.
 */
export function readPackage(root: string, folder: string, lockfile: LockFile): BundledPackage {
  const directory = path.join(root, folder);
  const manifest = JSON.parse(fs.readFileSync(path.join(directory, "package.json"), "utf8"));
  const { name, version, license } = manifest as Record<string, unknown>;
  const label = `${String(name)} ${String(version)}`;
  if (typeof license !== "string" || !ACCEPTED.includes(license)) {
    throw new Error(
      `${label} declares the license ${JSON.stringify(license)}, which is not accepted. Review it before adding it to ACCEPTED in scripts/notices.mts.`,
    );
  }

  const names = fs.readdirSync(directory).sort();
  const licenses = names.filter((file) => LICENSE_FILE.test(file));
  if (licenses.length === 0) {
    throw new Error(`${label} holds no license file`);
  }
  const files = [...licenses, ...names.filter((file) => NOTICE_FILE.test(file))].map((file) => ({
    name: file,
    text: fs.readFileSync(path.join(directory, file), "utf8").replace(/\r\n/g, "\n").trimEnd(),
  }));

  const source = lockfile.packages[folder]?.resolved;
  if (!source) {
    throw new Error(`package-lock.json gives no download address for ${folder}`);
  }
  return { name: String(name), version: String(version), license, source, files };
}

/** The part of `package-lock.json` that the notices read. */
export interface LockFile {
  packages: Record<string, { resolved?: string }>;
}

/** The text of THIRD_PARTY_LICENSES.txt for `packages`. */
export function formatNotices(packages: BundledPackage[]): string {
  const sorted = [...packages].sort((a, b) => a.name.localeCompare(b.name, "en"));
  const list = sorted.map(
    (item) =>
      `${item.name} ${item.version}\n  License: ${item.license}\n  Source: ${item.source}\n`,
  );
  const texts = sorted.flatMap((item) =>
    item.files.map(
      (file) => `\n${"=".repeat(72)}\n${item.name} ${item.version}: ${file.name}\n\n${file.text}\n`,
    ),
  );
  return [
    "Third-party licenses for the Translation LSP extension for VS Code\n\n",
    "out/extension.js holds code of the npm packages below, including the\n",
    "helpers that the bundler Rolldown adds. Each entry gives the license that\n",
    "the package declares and where it was downloaded from. The license files\n",
    "and notices of each package follow the list.\n\n",
    ...list,
    ...texts,
  ].join("");
}

/** Writes THIRD_PARTY_LICENSES.txt for the bundle that the last build wrote. */
function writeNotices(root: string): void {
  const out = path.join(root, "out");
  const map = JSON.parse(fs.readFileSync(path.join(out, "extension.js.map"), "utf8"));
  const lockfile = JSON.parse(fs.readFileSync(path.join(root, "package-lock.json"), "utf8"));
  const folders = listPackageFolders(map.sources, out, root);
  if (folders.length === 0) {
    throw new Error("The source map of out/extension.js names no npm package");
  }
  const packages = [...new Set([...folders, BUNDLER])].map((folder) =>
    readPackage(root, folder, lockfile),
  );
  fs.writeFileSync(path.join(root, "THIRD_PARTY_LICENSES.txt"), formatNotices(packages));
}

if (process.argv[1] && fs.realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  writeNotices(path.join(import.meta.dirname, ".."));
}
