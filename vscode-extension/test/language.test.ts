import { expect, test } from "vitest";
import { nameLanguage } from "../src/language";

test("a language code is named in the display language", () => {
  expect(nameLanguage("ja", "en")).toBe("Japanese");
  expect(nameLanguage("ja", "ja")).toBe("日本語");
  expect(nameLanguage("pt-BR", "en")).toBe("Brazilian Portuguese");
  expect(nameLanguage("zh-Hant", "en")).toBe("Traditional Chinese");
  expect(nameLanguage("zh-Hant", "ja")).toBe("繁体中国語");
});

test("a value that is no code with a name of its own shows as written", () => {
  // `Intl` reads `Japanese` as a code, and gives it back as its name.
  expect(nameLanguage("Japanese", "en")).toBe("Japanese");
  expect(nameLanguage("Japanese", "ja")).toBe("Japanese");
  // A code for private use has no name.
  expect(nameLanguage("qaa", "en")).toBe("qaa");
  for (const value of ["", "ja!", "a b", "日本語"]) {
    expect(nameLanguage(value, "en"), value).toBe(value);
  }
  // A display language that `Intl` cannot read names nothing either.
  expect(nameLanguage("ja", "not a locale!")).toBe("ja");
});
