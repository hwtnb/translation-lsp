import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { expect, onTestFinished, test } from "vitest";
import { formatNotices, listPackageFolders, readPackage } from "../scripts/notices.mjs";

/** A new folder that is removed after the test. */
function makeFolder(): string {
  const folder = fs.mkdtempSync(path.join(os.tmpdir(), "translation-lsp-notices-"));
  onTestFinished(() => fs.rmSync(folder, { recursive: true }));
  return folder;
}

/** Writes a package with these files into `node_modules` of `root`. */
function writePackage(root: string, folder: string, files: Record<string, string>): void {
  fs.mkdirSync(path.join(root, folder), { recursive: true });
  for (const [name, text] of Object.entries(files)) {
    fs.writeFileSync(path.join(root, folder, name), text);
  }
}

const LOCKFILE = {
  packages: {
    "node_modules/a": { resolved: "https://registry.npmjs.org/a/-/a-1.0.0.tgz" },
  },
};

test("the sources of the source map name each bundled package once", () => {
  const root = path.join(path.sep, "work", "extension");
  const out = path.join(root, "out");
  expect(
    listPackageFolders(
      [
        "../src/extension.ts",
        "../package.json",
        "../node_modules/a/lib/main.js",
        "../node_modules/a/lib/other.js",
        "../node_modules/@scope/b/index.js",
        "../node_modules/a/node_modules/c/index.js",
        path.join(root, "node_modules", "d", "index.js"),
      ],
      out,
      root,
    ),
  ).toStrictEqual([
    "node_modules/a",
    "node_modules/@scope/b",
    "node_modules/a/node_modules/c",
    "node_modules/d",
  ]);
  expect(() => listPackageFolders(["../../elsewhere/index.js"], out, root)).toThrow(/outside/);
  for (const source of ["../scripts/notices.mts", "../out/other.js", "webpack://a/index.js"]) {
    expect(() => listPackageFolders([source], out, root), source).toThrow(/neither/);
  }
});

test("a bundled package gives its license files and notices", () => {
  const root = makeFolder();
  writePackage(root, "node_modules/a", {
    "package.json": JSON.stringify({ name: "a", version: "1.0.0", license: "MIT" }),
    "License.txt": "MIT License\r\n\r\nCopyright A\r\n",
    "thirdpartynotices.txt": "Notices of A\n",
    "THIRD-PARTY-LICENSE": "Licenses of code in A\n",
    "README.md": "Not a license",
  });

  const bundled = readPackage(root, "node_modules/a", LOCKFILE);
  expect(bundled).toStrictEqual({
    name: "a",
    version: "1.0.0",
    license: "MIT",
    source: "https://registry.npmjs.org/a/-/a-1.0.0.tgz",
    files: [
      { name: "License.txt", text: "MIT License\n\nCopyright A" },
      { name: "THIRD-PARTY-LICENSE", text: "Licenses of code in A" },
      { name: "thirdpartynotices.txt", text: "Notices of A" },
    ],
  });
});

test("a package without a license file, or under a license not accepted, fails", () => {
  const root = makeFolder();
  writePackage(root, "node_modules/a", {
    "package.json": JSON.stringify({ name: "a", version: "1.0.0", license: "MIT" }),
  });
  expect(() => readPackage(root, "node_modules/a", LOCKFILE)).toThrow(/no license file/);

  for (const license of ["GPL-3.0-only", "(MIT OR Apache-2.0)", undefined, { type: "MIT" }]) {
    writePackage(root, "node_modules/a", {
      "package.json": JSON.stringify({ name: "a", version: "1.0.0", license }),
      LICENSE: "Some license",
    });
    expect(() => readPackage(root, "node_modules/a", LOCKFILE), JSON.stringify(license)).toThrow(
      /not accepted/,
    );
  }

  writePackage(root, "node_modules/b", {
    "package.json": JSON.stringify({ name: "b", version: "1.0.0", license: "ISC" }),
    LICENSE: "ISC License",
  });
  expect(() => readPackage(root, "node_modules/b", LOCKFILE)).toThrow(/download address/);
});

test("the notices list the packages by name before their license texts", () => {
  const text = formatNotices([
    {
      name: "b",
      version: "2.0.0",
      license: "ISC",
      source: "https://example.com/b.tgz",
      files: [{ name: "LICENSE", text: "ISC text" }],
    },
    {
      name: "a",
      version: "1.0.0",
      license: "MIT",
      source: "https://example.com/a.tgz",
      files: [{ name: "LICENSE.md", text: "MIT text" }],
    },
  ]);
  expect(
    text.indexOf("a 1.0.0\n  License: MIT\n  Source: https://example.com/a.tgz\n"),
  ).toBeLessThan(text.indexOf("b 2.0.0\n  License: ISC\n"));
  expect(text.indexOf("a 1.0.0: LICENSE.md\n\nMIT text\n")).toBeLessThan(
    text.indexOf("b 2.0.0: LICENSE\n\nISC text\n"),
  );
  expect(text.endsWith("ISC text\n")).toBe(true);
});
