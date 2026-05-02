/// i18n bootstrap. Wraps preact-i18n with an `en` default; new locales can be
/// dropped in alongside `en.json`.

import en from "./en.json";

/** All available locale codes → their human-readable label. */
export const LOCALES: Record<string, string> = {
  en: "English",
};

/** Translation dictionaries keyed by locale code. */
const DEFINITIONS: Record<string, Record<string, unknown>> = {
  en,
};

export function getDefinition(locale: string): Record<string, unknown> {
  return DEFINITIONS[locale] ?? en;
}

export const defaultLocale = "en";
