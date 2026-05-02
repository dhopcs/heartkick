/// Integrations page: configure webhooks, prometheus, OSC, overlay server, etc.

import { useEffect, useRef, useState } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { api } from "../api";
import type { AppConfig, WebhookConfig } from "../types";
import { CodeEditor } from "../components/CodeEditor";

const emptyWebhook = (): WebhookConfig => ({
  name: "webhook",
  enabled: false,
  method: "POST",
  url: "",
  headers: {},
  body: '{"bpm":{bpm},"timestamp":"{timestamp}"}',
  min_interval_ms: 1000,
});

export function IntegrationsPage() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  useEffect(() => {
    api.getConfig().then(setConfig);
  }, []);

  if (!config) return <div class="text-sm text-white/50">Loading…</div>;

  const update = (fn: (c: AppConfig) => AppConfig) => setConfig((c) => (c ? fn(c) : c));

  async function save() {
    if (!config) return;
    await api.saveConfig(config);
    setSavedAt(Date.now());
  }

  return (
    <div class="space-y-6 pb-20">
      <h2 class="text-lg font-semibold">Integrations</h2>

      {/* Overlay */}
      <SubSection title="Overlay">
        <Toggle
          label="Enable overlay server"
          value={config.integrations.overlay.enabled}
          onChange={(v) =>
            update((c) => ({
              ...c,
              integrations: {
                ...c.integrations,
                overlay: { ...c.integrations.overlay, enabled: v },
              },
            }))
          }
        />
        <Field label="Bind address">
          <input
            class="w-full rounded-md bg-white/10 px-3 py-1.5 font-mono text-sm"
            value={config.integrations.overlay.bind}
            onInput={(e) =>
              update((c) => ({
                ...c,
                integrations: {
                  ...c.integrations,
                  overlay: { ...c.integrations.overlay, bind: e.currentTarget.value },
                },
              }))
            }
          />
        </Field>
        {config.integrations.overlay.enabled && (
          <p class="text-xs text-white/40">
            Browser source URL:{" "}
            <a
              class="font-mono text-white/60 underline decoration-white/20 hover:text-white"
              href={`http://${config.integrations.overlay.bind}/`}
              target="_blank"
              rel="noopener noreferrer"
            >
              http://{config.integrations.overlay.bind}/
            </a>
            <span class="ml-2 text-amber-400/80">
              (save &amp; restart the app to apply changes)
            </span>
          </p>
        )}
        <OverlayHtmlEditor />
      </SubSection>

      {/* Prometheus */}
      <SubSection title="Prometheus">
        <Toggle
          label="Serve metrics server (/metrics)"
          value={config.integrations.prometheus.enabled}
          onChange={(v) =>
            update((c) => ({
              ...c,
              integrations: {
                ...c.integrations,
                prometheus: { ...c.integrations.prometheus, enabled: v },
              },
            }))
          }
        />
        {config.integrations.prometheus.enabled && (
          <>
            <Field label="Bind address">
              <input
                class="w-full rounded-md bg-white/10 px-3 py-1.5 font-mono text-sm"
                value={config.integrations.prometheus.bind}
                onInput={(e) =>
                  update((c) => ({
                    ...c,
                    integrations: {
                      ...c.integrations,
                      prometheus: { ...c.integrations.prometheus, bind: e.currentTarget.value },
                    },
                  }))
                }
              />
            </Field>
            <p class="text-xs text-white/40">
              Scrape endpoint:{" "}
              <span class="font-mono">http://{config.integrations.prometheus.bind}/metrics</span>
            </p>
          </>
        )}

        <Toggle
          label="Push metrics (POST)"
          value={config.integrations.prometheus.push?.enabled ?? false}
          onChange={(v) =>
            update((c) => ({
              ...c,
              integrations: {
                ...c.integrations,
                prometheus: {
                  ...c.integrations.prometheus,
                  push: { url: "", headers: {}, ...c.integrations.prometheus.push, enabled: v },
                },
              },
            }))
          }
        />
        {config.integrations.prometheus.push?.enabled && (
          <>
            <Field label="Push URL">
              <input
                class="w-full rounded-md bg-white/10 px-3 py-1.5 font-mono text-sm"
                placeholder="http://victoria:8428/api/v1/import/prometheus"
                value={config.integrations.prometheus.push?.url ?? ""}
                onInput={(e) =>
                  update((c) => ({
                    ...c,
                    integrations: {
                      ...c.integrations,
                      prometheus: {
                        ...c.integrations.prometheus,
                        push: { ...c.integrations.prometheus.push!, url: e.currentTarget.value },
                      },
                    },
                  }))
                }
              />
            </Field>
            <Field label="Extra headers (one per line, Key: Value)">
              <HeadersTextarea
                headers={config.integrations.prometheus.push?.headers ?? {}}
                onChange={(headers) =>
                  update((c) => ({
                    ...c,
                    integrations: {
                      ...c.integrations,
                      prometheus: {
                        ...c.integrations.prometheus,
                        push: { ...c.integrations.prometheus.push!, headers },
                      },
                    },
                  }))
                }
              />
            </Field>
            <p class="text-xs text-white/30">
              Metrics are POSTed in Prometheus text format on every heart rate sample. Compatible
              with VictoriaMetrics, Grafana Alloy, and any endpoint that accepts{" "}
              <span class="font-mono">text/plain</span> Prometheus exposition.
            </p>
          </>
        )}
      </SubSection>

      {/* OSC */}
      <SubSection title="OSC">
        <Toggle
          label="Enable OSC output"
          value={config.integrations.osc.enabled}
          onChange={(v) =>
            update((c) => ({
              ...c,
              integrations: { ...c.integrations, osc: { ...c.integrations.osc, enabled: v } },
            }))
          }
        />
        <Field label="Target (host:port)">
          <input
            class="w-full rounded-md bg-white/10 px-3 py-1.5 font-mono text-sm"
            value={config.integrations.osc.target}
            onInput={(e) =>
              update((c) => ({
                ...c,
                integrations: {
                  ...c.integrations,
                  osc: { ...c.integrations.osc, target: e.currentTarget.value },
                },
              }))
            }
          />
        </Field>
        <Field label="OSC address">
          <input
            class="w-full rounded-md bg-white/10 px-3 py-1.5 font-mono text-sm"
            value={config.integrations.osc.address}
            onInput={(e) =>
              update((c) => ({
                ...c,
                integrations: {
                  ...c.integrations,
                  osc: { ...c.integrations.osc, address: e.currentTarget.value },
                },
              }))
            }
          />
        </Field>
      </SubSection>

      {/* Webhooks */}
      <SubSection title={`Webhooks (${config.integrations.webhooks.length})`}>
        {config.integrations.webhooks.map((w, i) => (
          <div key={i} class="rounded-lg bg-white/5 p-3 space-y-2">
            <div class="flex items-center justify-between gap-2">
              <input
                class="flex-1 rounded-md bg-white/10 px-2 py-1 text-sm"
                value={w.name}
                onInput={(e) =>
                  update((c) => {
                    const wh = [...c.integrations.webhooks];
                    wh[i] = { ...wh[i], name: e.currentTarget.value };
                    return { ...c, integrations: { ...c.integrations, webhooks: wh } };
                  })
                }
              />
              <Toggle
                label=""
                value={w.enabled}
                onChange={(v) =>
                  update((c) => {
                    const wh = [...c.integrations.webhooks];
                    wh[i] = { ...wh[i], enabled: v };
                    return { ...c, integrations: { ...c.integrations, webhooks: wh } };
                  })
                }
              />
              <button
                class="rounded-md bg-white/10 px-2 py-1 text-xs hover:bg-red-500/20 hover:text-red-400 transition-colors"
                onClick={() =>
                  update((c) => ({
                    ...c,
                    integrations: {
                      ...c.integrations,
                      webhooks: c.integrations.webhooks.filter((_, j) => j !== i),
                    },
                  }))
                }
              >
                Remove
              </button>
            </div>
            <div class="grid grid-cols-[5rem_1fr] gap-2">
              <select
                class="rounded-md bg-white/10 px-2 py-1 text-sm"
                value={w.method}
                onChange={(e) =>
                  update((c) => {
                    const wh = [...c.integrations.webhooks];
                    wh[i] = { ...wh[i], method: e.currentTarget.value };
                    return { ...c, integrations: { ...c.integrations, webhooks: wh } };
                  })
                }
              >
                {["GET", "POST", "PUT", "PATCH"].map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
              <input
                class="rounded-md bg-white/10 px-2 py-1 text-sm"
                placeholder="https://example.com/hook"
                value={w.url}
                onInput={(e) =>
                  update((c) => {
                    const wh = [...c.integrations.webhooks];
                    wh[i] = { ...wh[i], url: e.currentTarget.value };
                    return { ...c, integrations: { ...c.integrations, webhooks: wh } };
                  })
                }
              />
            </div>
            <textarea
              class="h-20 w-full rounded-md bg-white/10 px-2 py-1 font-mono text-xs"
              value={w.body}
              onInput={(e) =>
                update((c) => {
                  const wh = [...c.integrations.webhooks];
                  wh[i] = { ...wh[i], body: e.currentTarget.value };
                  return { ...c, integrations: { ...c.integrations, webhooks: wh } };
                })
              }
            />
            <div class="flex items-center gap-2 text-xs text-white/40">
              <span>Variables: {`{bpm} {rr} {timestamp} {device}`}</span>
              <span class="ml-auto">min interval</span>
              <input
                type="number"
                class="w-20 rounded bg-white/10 px-1 py-0.5 text-right text-xs"
                value={w.min_interval_ms}
                onInput={(e) =>
                  update((c) => {
                    const wh = [...c.integrations.webhooks];
                    wh[i] = { ...wh[i], min_interval_ms: Number(e.currentTarget.value) || 0 };
                    return { ...c, integrations: { ...c.integrations, webhooks: wh } };
                  })
                }
              />
              <span>ms</span>
            </div>
          </div>
        ))}
        <button
          class="rounded-md bg-white/10 px-3 py-1.5 text-sm transition-colors hover:bg-white/15"
          onClick={() =>
            update((c) => ({
              ...c,
              integrations: {
                ...c.integrations,
                webhooks: [...c.integrations.webhooks, emptyWebhook()],
              },
            }))
          }
        >
          + Add webhook
        </button>
      </SubSection>

      {/* Sticky save */}
      <div class="sticky bottom-4 z-10 flex items-center justify-end gap-3">
        {savedAt && Date.now() - savedAt < 3000 && (
          <span class="text-xs text-emerald-400">Saved!</span>
        )}
        <button
          class="rounded-lg px-5 py-2.5 text-sm font-semibold text-white shadow-lg transition-opacity hover:opacity-90"
          style={{ background: "var(--color-pulse)" }}
          onClick={save}
        >
          Save
        </button>
      </div>
    </div>
  );
}

function OverlayHtmlEditor() {
  const [html, setHtml] = useState<string | null>(null); // null = loading
  const [defaultHtml, setDefaultHtml] = useState("");
  const [isCustom, setIsCustom] = useState(false);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  useEffect(() => {
    Promise.all([api.getOverlayHtml(), api.getDefaultOverlayHtml()]).then(([custom, def]) => {
      setDefaultHtml(def);
      if (custom !== null) {
        setHtml(custom);
        setIsCustom(true);
      } else {
        setHtml(def);
        setIsCustom(false);
      }
    });
  }, []);

  async function save() {
    if (html === null) return;
    await api.saveOverlayHtml(html);
    setIsCustom(true);
    setSavedAt(Date.now());
  }

  async function reset() {
    await api.resetOverlayHtml();
    setHtml(defaultHtml);
    setIsCustom(false);
    setSavedAt(Date.now());
  }

  if (html === null) return <div class="text-xs text-white/40">Loading…</div>;

  return (
    <div class="space-y-2">
      <div class="flex items-center justify-between">
        <span class="text-sm text-white/70">
          Overlay HTML
          {isCustom && (
            <span class="ml-2 rounded bg-amber-500/20 px-1.5 py-0.5 text-xs text-amber-400">
              customised
            </span>
          )}
        </span>
        <div class="flex gap-2">
          {isCustom && (
            <button
              class="rounded-md bg-white/10 px-2.5 py-1 text-xs transition-colors hover:bg-white/15"
              onClick={reset}
            >
              Reset to default
            </button>
          )}
          <button
            class="rounded-md px-2.5 py-1 text-xs font-medium text-white transition-opacity hover:opacity-80"
            style={{ background: "var(--color-pulse)" }}
            onClick={save}
          >
            {savedAt && Date.now() - savedAt < 2000 ? "Saved!" : "Save HTML"}
          </button>
        </div>
      </div>
      <CodeEditor value={html} onChange={setHtml} />
      <p class="text-xs text-white/30">
        Changes take effect on the next browser source refresh. The page polls{" "}
        <span class="font-mono">/api/bpm</span> every 500 ms automatically.
      </p>
    </div>
  );
}

// ── Shared UI helpers ─────────────────────────────────────────────────────────

/** Uncontrolled textarea for editing `Record<string,string>` headers.
 *  Keeps a local draft so typing is never interrupted by round-trip serialization. */
function HeadersTextarea({
  headers,
  onChange,
}: {
  headers: Record<string, string>;
  onChange: (h: Record<string, string>) => void;
}) {
  const serialize = (h: Record<string, string>) =>
    Object.entries(h)
      .map(([k, v]) => `${k}: ${v}`)
      .join("\n");

  const [draft, setDraft] = useState(() => serialize(headers));

  // Sync inward only when the prop changes externally (e.g. section toggle resets it).
  const prevHeaders = useRef(headers);
  if (prevHeaders.current !== headers) {
    prevHeaders.current = headers;
    const next = serialize(headers);
    if (next !== draft) setDraft(next);
  }

  function commit(text: string) {
    const parsed: Record<string, string> = {};
    for (const line of text.split("\n")) {
      const idx = line.indexOf(":");
      if (idx > 0) parsed[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
    }
    onChange(parsed);
  }

  return (
    <textarea
      class="h-20 w-full rounded-md bg-white/10 px-3 py-1.5 font-mono text-xs"
      placeholder={"Authorization: Bearer token\nX-Custom: value"}
      value={draft}
      onInput={(e) => setDraft(e.currentTarget.value)}
      onBlur={(e) => commit(e.currentTarget.value)}
    />
  );
}

function SubSection({ title, children }: { title: string; children: ComponentChildren }) {
  const [open, setOpen] = useState(true);
  return (
    <div class="rounded-xl bg-white/[0.03] overflow-hidden">
      <button
        class="flex w-full items-center justify-between px-4 py-3 text-sm font-medium text-white/70 hover:text-white transition-colors"
        onClick={() => setOpen((v) => !v)}
      >
        <span>{title}</span>
        <span class="text-white/30 text-xs">{open ? "▲" : "▼"}</span>
      </button>
      {open && <div class="px-4 pb-4 space-y-3">{children}</div>}
    </div>
  );
}

function Field({ label, children }: { label: string; children: ComponentChildren }) {
  return (
    <label class="block space-y-1">
      <span class="text-sm text-white/70">{label}</span>
      {children}
    </label>
  );
}

function Toggle({
  label,
  value,
  onChange,
}: {
  label: string;
  value: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label class="flex items-center justify-between gap-3">
      {label && <span class="text-sm text-white/70">{label}</span>}
      <button
        type="button"
        class="relative h-6 w-11 shrink-0 rounded-full transition-colors"
        style={{ background: value ? "var(--color-pulse)" : "rgba(255,255,255,0.15)" }}
        onClick={() => onChange(!value)}
        aria-pressed={value}
      >
        <span
          class="absolute left-0.5 top-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform duration-150"
          style={{ transform: value ? "translateX(1.25rem)" : "translateX(0)" }}
        />
      </button>
    </label>
  );
}
