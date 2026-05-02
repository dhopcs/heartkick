/// <reference types="vite/client" />

declare module "preact-i18n" {
  import type { ComponentChildren, FunctionComponent } from "preact";

  export interface IntlProviderProps {
    definition: Record<string, unknown>;
    scope?: string;
    children?: ComponentChildren;
  }
  export const IntlProvider: FunctionComponent<IntlProviderProps>;

  export interface TextProps {
    id: string;
    fields?: Record<string, unknown>;
    plural?: number;
    children?: ComponentChildren;
  }
  export const Text: FunctionComponent<TextProps>;

  export function withText(mapping: unknown): <P>(c: P) => P;
}
