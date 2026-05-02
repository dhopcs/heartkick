/// Lightweight code editor backed by CodeJar + Prism (HTML/CSS/JS grammar).

import { useEffect, useRef } from "preact/hooks";
import { CodeJar } from "codejar";
import Prism from "prismjs";
// Base prismjs bundle already includes markup (HTML), css, and javascript.
// Okaidia is a dark theme that fits the app well.
import "prismjs/themes/prism-okaidia.min.css";

interface Props {
  value: string;
  onChange: (v: string) => void;
  /** Prism language key, defaults to "markup" (HTML). */
  language?: string;
  class?: string;
  style?: Record<string, string | number>;
}

export function CodeEditor({ value, onChange, language = "markup", class: cls, style }: Props) {
  const preRef = useRef<HTMLPreElement>(null);
  const jarRef = useRef<ReturnType<typeof CodeJar> | null>(null);
  // Track what value CodeJar currently holds so we only call updateCode on
  // external changes (e.g. a "Reset to default" action), not on every keystroke.
  const jarValue = useRef(value);

  // Initialize CodeJar once.
  useEffect(() => {
    const pre = preRef.current;
    if (!pre) return;

    function highlight(el: HTMLElement) {
      const code = el.textContent ?? "";
      const grammar = Prism.languages[language] ?? Prism.languages.markup;
      el.innerHTML = Prism.highlight(code, grammar, language);
    }

    const jar = CodeJar(pre, highlight, { tab: "  ", indentOn: /[({[<]$/ });
    jar.updateCode(value);
    jarValue.current = value;

    jar.onUpdate((code) => {
      jarValue.current = code;
      onChange(code);
    });

    jarRef.current = jar;
    return () => jar.destroy();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Sync external changes to CodeJar
  useEffect(() => {
    if (jarRef.current && value !== jarValue.current) {
      jarRef.current.updateCode(value);
      jarValue.current = value;
    }
  }, [value]);

  return (
    <div
      class={cls ?? ""}
      style={{
        height: "14rem",
        minHeight: "6rem",
        resize: "vertical",
        overflow: "auto",
        borderRadius: "0.5rem",
        boxShadow: "0 0 0 1px rgba(255,255,255,0.08)",
        background: "rgba(0,0,0,0.45)",
        ...style,
      }}
    >
      <pre
        ref={preRef}
        class={`language-${language} codejar-wrap`}
        style={{
          margin: 0,
          padding: "0.625rem 0.75rem",
          fontSize: "0.75rem",
          lineHeight: "1.5",
          minHeight: "100%",
          boxSizing: "border-box",
          overflow: "visible",
          background: "transparent",
          outline: "none",
          whiteSpace: "pre",
          wordBreak: "normal",
          overflowWrap: "normal",
          tabSize: 2,
        }}
      />
    </div>
  );
}
