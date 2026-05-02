/// Settings page. Exposes the live config and persists changes through the
/// `save_config` Tauri command.

import { useEffect, useState } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { api } from "../api";
import { LOCALES } from "../i18n";
import type { AppConfig } from "../types";

interface Props {
  onLocaleChange?: (locale: string) => void;
}

export function SettingsPage({ onLocaleChange }: Props) {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [paths, setPaths] = useState<{ config: string; data: string } | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  useEffect(() => {
    api.getConfig().then(setConfig);
    api.configPaths().then(setPaths);
  }, []);

  if (!config) return <div class="text-sm text-white/50">Loading…</div>;

  const update = (fn: (c: AppConfig) => AppConfig) => setConfig((c) => (c ? fn(c) : c));

  async function save() {
    if (!config) return;
    await api.saveConfig(config);
    setSavedAt(Date.now());
    onLocaleChange?.(config.general.locale);
  }

  return (
    <div class="space-y-6 pb-20">
      <h2 class="text-lg font-semibold">Settings</h2>

      {/* General */}
      <Section title="General">
        <Field label="Language">
          <select
            class="w-full rounded-md bg-white/10 px-3 py-1.5 text-sm"
            value={config.general.locale}
            onChange={(e) =>
              update((c) => ({
                ...c,
                general: { ...c.general, locale: e.currentTarget.value },
              }))
            }
          >
            {Object.entries(LOCALES).map(([code, label]) => (
              <option key={code} value={code}>
                {label}
              </option>
            ))}
          </select>
        </Field>
      </Section>

      {/* Bluetooth */}
      <Section title="Bluetooth">
        <Field label="Saved device address">
          <input
            class="w-full rounded-md bg-white/10 px-3 py-1.5 font-mono text-sm"
            placeholder="AA:BB:CC:DD:EE:FF"
            value={config.bluetooth.device_address ?? ""}
            onInput={(e) =>
              update((c) => ({
                ...c,
                bluetooth: { ...c.bluetooth, device_address: e.currentTarget.value || null },
              }))
            }
          />
        </Field>
        <Toggle
          label="Auto-reconnect on disconnect"
          value={config.bluetooth.auto_reconnect}
          onChange={(v) =>
            update((c) => ({ ...c, bluetooth: { ...c.bluetooth, auto_reconnect: v } }))
          }
        />
      </Section>

      {/* REST API */}
      <Section title="HTTP API">
        <Toggle
          label="Enable HTTP API"
          value={config.api.http_enabled}
          onChange={(v) => update((c) => ({ ...c, api: { ...c.api, http_enabled: v } }))}
        />
        <Field label="Bind address">
          <input
            class="w-full rounded-md bg-white/10 px-3 py-1.5 font-mono text-sm"
            value={config.api.http_bind}
            onInput={(e) =>
              update((c) => ({ ...c, api: { ...c.api, http_bind: e.currentTarget.value } }))
            }
          />
        </Field>
        <Toggle
          label="Enable IPC socket"
          value={config.api.socket_enabled}
          onChange={(v) => update((c) => ({ ...c, api: { ...c.api, socket_enabled: v } }))}
        />
        <Field label="API token">
          <div class="flex gap-2">
            <input
              class="flex-1 rounded-md bg-white/10 px-3 py-1.5 font-mono text-sm"
              type="text"
              placeholder="Leave empty to disable auth"
              value={config.api.api_token ?? ""}
              onInput={(e) =>
                update((c) => ({
                  ...c,
                  api: { ...c.api, api_token: e.currentTarget.value || null },
                }))
              }
            />
            <button
              class="shrink-0 rounded-md bg-white/10 px-3 py-1.5 text-sm transition-colors hover:bg-white/15"
              type="button"
              onClick={() => {
                const token = Array.from(crypto.getRandomValues(new Uint8Array(24)))
                  .map((b) => b.toString(16).padStart(2, "0"))
                  .join("");
                update((c) => ({ ...c, api: { ...c.api, api_token: token } }));
              }}
            >
              Generate
            </button>
          </div>
          <p class="text-xs text-white/30 mt-1">
            When set, all requests must include{" "}
            <span class="font-mono">Authorization: Bearer &lt;token&gt;</span>
          </p>
        </Field>
      </Section>

      {/* File paths */}
      {paths && (
        <div class="rounded-xl bg-white/5 p-3 text-xs text-white/40 space-y-1">
          <div>
            Config: <span class="font-mono text-white/60">{paths.config}</span>
          </div>
          <div>
            Data: <span class="font-mono text-white/60">{paths.data}</span>
          </div>
        </div>
      )}

      {/* Sticky save button */}
      <div class="sticky bottom-4 z-10 flex items-center justify-end gap-3">
        {savedAt && Date.now() - savedAt < 3000 && (
          <span class="text-xs text-emerald-400">Saved!</span>
        )}
        <button
          class="rounded-lg px-5 py-2.5 text-sm font-semibold text-white shadow-lg transition-opacity hover:opacity-90"
          style={{ background: "var(--color-pulse)" }}
          onClick={save}
        >
          Save settings
        </button>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ComponentChildren }) {
  return (
    <div class="space-y-3 rounded-xl bg-white/[0.03] p-4">
      <h3 class="text-sm font-semibold uppercase tracking-wider text-white/60">{title}</h3>
      <div class="space-y-3">{children}</div>
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
