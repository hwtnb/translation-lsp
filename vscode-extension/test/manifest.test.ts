import * as fs from "node:fs";
import * as path from "node:path";
import { expect, test } from "vitest";
import manifest from "../package.json";
import english from "../package.nls.json";
import japanese from "../package.nls.ja.json";
import { PROVIDER_FIELDS, SECTION, toServerName } from "../src/settings";
import bundle from "../l10n/bundle.l10n.ja.json";

const root = path.join(import.meta.dirname, "..");

/** The display name of each provider, as the settings show it. */
const PROVIDERS: Record<string, string> = {
  chatgpt: "ChatGPT",
  anthropic_api: "Anthropic API",
  openai_api: "OpenAI API",
  openai_compatible: "OpenAI-compatible API",
  deepl: "DeepL API",
  google_translate: "Google Cloud Translation",
  plamo_translate: "PLaMo Translate",
};

const properties: Record<string, Record<string, unknown>> = Object.assign(
  {},
  ...manifest.contributes.configuration.map((group) => group.properties),
);

test("every localized key of the manifest is in both languages, and no other", () => {
  const used = new Set(
    JSON.stringify(manifest)
      .match(/%[^%"]+%/g)
      ?.map((key) => key.slice(1, -1)),
  );
  expect(new Set(Object.keys(english))).toStrictEqual(used);
  expect(new Set(Object.keys(japanese))).toStrictEqual(used);
});

test("every text of the extension has a Japanese translation, and no other", () => {
  const source = fs
    .readdirSync(path.join(root, "src"))
    .filter((file) => file.endsWith(".ts"))
    .map((file) => fs.readFileSync(path.join(root, "src", file), "utf8"))
    .join("\n");
  const texts = new Set(
    [...source.matchAll(/l10n\.t\(\s*"((?:[^"\\]|\\.)*)"/g)].map((match) =>
      JSON.parse(`"${match[1]}"`),
    ),
  );
  expect(texts.size).toBeGreaterThan(0);
  expect(new Set(Object.keys(bundle))).toStrictEqual(texts);
});

test("the providers of the settings are those the extension sends settings for", () => {
  const name = properties[`${SECTION}.provider.name`] as {
    enum: string[];
    enumItemLabels: string[];
  };
  expect(name.enum).toStrictEqual(["", ...Object.keys(PROVIDER_FIELDS)]);
  expect(name.enumItemLabels.slice(1)).toStrictEqual(
    Object.keys(PROVIDER_FIELDS).map((provider) => PROVIDERS[provider]),
  );
});

test("each provider setting says which providers use it", () => {
  for (const [key, schema] of Object.entries(properties)) {
    const field = toServerName(key.slice(`${SECTION}.provider.`.length));
    if (!key.startsWith(`${SECTION}.provider.`) || field === "name") {
      continue;
    }

    const description =
      english[(schema.markdownDescription as string).slice(1, -1) as keyof typeof english];
    const users = Object.entries(PROVIDER_FIELDS)
      .filter(([, fields]) => fields.includes(field))
      .map(([provider]) => PROVIDERS[provider]);
    expect(users.length, key).toBeGreaterThan(0);

    const sentence = description.slice(description.lastIndexOf("Used by "));
    for (const [provider, display] of Object.entries(PROVIDERS)) {
      expect(sentence.includes(display), `${key}: ${provider}`).toBe(users.includes(display));
    }
  }
});

test("every command of a menu is contributed", () => {
  const commands = new Set(manifest.contributes.commands.map((command) => command.command));
  for (const items of Object.values(manifest.contributes.menus)) {
    for (const item of items) {
      expect(commands.has(item.command), item.command).toBe(true);
    }
  }
});
