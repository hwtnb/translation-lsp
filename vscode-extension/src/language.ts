// The name of the language that the server translates into. Nothing here uses
// the VS Code API, so the tests run it without VS Code.

/**
 * The name of the target language `value` in `displayLanguage`, if `value`
 * is a language code that has one, or `value` as it is: a name such as
 * `Japanese`, a code without a name of its own, or text that is no code.
 */
export function nameLanguage(value: string, displayLanguage: string): string {
  try {
    const name = new Intl.DisplayNames([displayLanguage], { type: "language" }).of(value);
    return name && name.toLowerCase() !== value.toLowerCase() ? name : value;
  } catch {
    // `Intl` refuses text that is no language code.
    return value;
  }
}
