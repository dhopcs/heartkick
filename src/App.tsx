import { useEffect, useState } from "preact/hooks";
import { IntlProvider } from "preact-i18n";
import { useEngine } from "./hooks/useEngine";
import { defaultLocale, getDefinition, LOCALES } from "./i18n";
import { api } from "./api";
import { HomePage } from "./pages/HomePage";
import { MetricsPage } from "./pages/MetricsPage";
import { SettingsPage } from "./pages/SettingsPage";
import { LogsPage } from "./pages/LogsPage";
import { IntegrationsPage } from "./pages/IntegrationsPage";
import { DevicesPage } from "./pages/DevicesPage";

import type { ComponentChildren } from "preact";

type Tab = "home" | "metrics" | "devices" | "integrations" | "settings" | "logs";

function IconHeart() {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" width="18" height="18">
      <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" />
    </svg>
  );
}

function IconChart() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.75"
      stroke-linecap="round"
      stroke-linejoin="round"
      width="18"
      height="18"
    >
      <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
    </svg>
  );
}

function IconDevices() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.75"
      stroke-linecap="round"
      stroke-linejoin="round"
      width="18"
      height="18"
    >
      <path d="M12 2C8.13 2 5 5.13 5 9c0 5.25 7 13 7 13s7-7.75 7-13c0-3.87-3.13-7-7-7z" />
      <circle cx="12" cy="9" r="2.5" />
    </svg>
  );
}

function IconIntegrations() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.75"
      stroke-linecap="round"
      stroke-linejoin="round"
      width="18"
      height="18"
    >
      <circle cx="12" cy="5" r="2" />
      <circle cx="5" cy="19" r="2" />
      <circle cx="19" cy="19" r="2" />
      <line x1="12" y1="7" x2="12" y2="14" />
      <line x1="12" y1="14" x2="5" y2="17" />
      <line x1="12" y1="14" x2="19" y2="17" />
    </svg>
  );
}

function IconSettings() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.75"
      stroke-linecap="round"
      stroke-linejoin="round"
      width="18"
      height="18"
    >
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  );
}

function IconLogs() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.75"
      stroke-linecap="round"
      stroke-linejoin="round"
      width="18"
      height="18"
    >
      <line x1="8" y1="6" x2="21" y2="6" />
      <line x1="8" y1="12" x2="21" y2="12" />
      <line x1="8" y1="18" x2="21" y2="18" />
      <line x1="3" y1="6" x2="3.01" y2="6" />
      <line x1="3" y1="12" x2="3.01" y2="12" />
      <line x1="3" y1="18" x2="3.01" y2="18" />
    </svg>
  );
}

const TABS: { id: Tab; icon: ComponentChildren; label: string }[] = [
  { id: "home", icon: <IconHeart />, label: "Home" },
  { id: "metrics", icon: <IconChart />, label: "Metrics" },
  { id: "devices", icon: <IconDevices />, label: "Devices" },
  { id: "integrations", icon: <IconIntegrations />, label: "Integrations" },
  { id: "settings", icon: <IconSettings />, label: "Settings" },
  { id: "logs", icon: <IconLogs />, label: "Logs" },
];

export function HeartIcon({ size = 20 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="var(--color-pulse)"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path d="M50 85 C50 85 10 55 10 30 C10 16 21 8 30 8 C38 8 45 13 50 20 C55 13 62 8 70 8 C79 8 90 16 90 30 C90 55 50 85 50 85Z" />
    </svg>
  );
}

function App() {
  const [tab, setTab] = useState<Tab>("home");
  const { snapshot, recent } = useEngine();
  const [locale, setLocale] = useState(defaultLocale);

  // Hydrate locale from persisted config once on mount.
  useEffect(() => {
    api
      .getConfig()
      .then((c) => {
        if (c.general.locale && c.general.locale in LOCALES) {
          setLocale(c.general.locale);
        }
      })
      .catch(() => {
        /* ignore */
      });
  }, []);

  return (
    <IntlProvider definition={getDefinition(locale)} scope={locale}>
      <div class="flex h-screen overflow-hidden">
        {/* Sidebar */}
        <aside class="hidden md:flex w-52 shrink-0 flex-col border-r border-white/5 bg-black/30">
          <div class="flex items-center gap-2.5 px-4 py-5">
            <HeartIcon size={20} />
            <span class="text-base font-semibold tracking-tight">heartkick</span>
          </div>
          <nav class="flex flex-col gap-0.5 px-2 flex-1">
            {TABS.map((t) => (
              <button
                key={t.id}
                class={`flex items-center gap-3 rounded-lg px-3 py-2 text-left text-sm transition-colors ${
                  tab === t.id
                    ? "bg-white/10 text-white"
                    : "text-white/50 hover:bg-white/5 hover:text-white/80"
                }`}
                onClick={() => setTab(t.id)}
              >
                <span class="w-5 text-center text-base leading-none">{t.icon}</span>
                <span>{t.label}</span>
              </button>
            ))}
          </nav>
          {/* Connection badge in sidebar footer */}
          {snapshot?.state === "connected" ? (
            <div class="mx-3 mb-4 rounded-lg bg-emerald-500/10 px-3 py-2 text-xs text-emerald-400">
              <div class="flex items-center justify-between gap-2">
                <div class="font-medium">Connected</div>
                <button
                  class="rounded px-1.5 py-0.5 text-[10px] text-emerald-300/70 hover:bg-emerald-500/20 transition-colors"
                  onClick={() => api.disconnect()}
                >
                  Disconnect
                </button>
              </div>
              {snapshot.device_address && (
                <div class="mt-0.5 truncate opacity-70">{snapshot.device_address}</div>
              )}
            </div>
          ) : (
            <button
              class={`mx-3 mb-4 w-[calc(100%-1.5rem)] rounded-lg px-3 py-2 text-left text-xs transition-colors ${
                snapshot?.state === "connecting" || snapshot?.state === "scanning"
                  ? "bg-amber-500/10 text-amber-400 hover:bg-amber-500/15"
                  : "bg-white/5 text-white/40 hover:bg-white/8 hover:text-white/60"
              }`}
              onClick={() => setTab("devices")}
            >
              <div class="font-medium capitalize">{snapshot?.state ?? "Disconnected"}</div>
              <div class="mt-0.5 opacity-60">Tap to manage devices</div>
            </button>
          )}
        </aside>

        {/* Main content area */}
        <main class="flex-1 overflow-y-auto" style={{ paddingTop: "env(safe-area-inset-top)" }}>
          <div
            class="mx-auto max-w-5xl px-4 pt-5 md:pb-8"
            style={{ paddingBottom: "calc(6rem + env(safe-area-inset-bottom))" }}
          >
            {tab === "home" && (
              <HomePage snapshot={snapshot} onGoToDevices={() => setTab("devices")} />
            )}
            {tab === "metrics" && <MetricsPage snapshot={snapshot} recent={recent} />}
            {tab === "devices" && <DevicesPage snapshot={snapshot} />}
            {tab === "integrations" && <IntegrationsPage />}
            {tab === "settings" && <SettingsPage onLocaleChange={setLocale} />}
            {tab === "logs" && <LogsPage />}
          </div>
        </main>

        {/* Mobile bottom nav */}
        <nav
          class="fixed inset-x-0 bottom-0 z-20 flex md:hidden"
          style={{
            background: "#0a0a0f",
            borderTop: "1px solid rgba(255,255,255,0.1)",
            paddingBottom: "env(safe-area-inset-bottom)",
          }}
        >
          {TABS.map((t) => (
            <button
              key={t.id}
              class={`flex flex-1 flex-col items-center gap-1 py-3 text-xs transition-colors ${
                tab === t.id ? "text-white" : "text-white/40"
              }`}
              onClick={() => setTab(t.id)}
            >
              <span class="text-xl leading-none">{t.icon}</span>
              <span class="text-[10px] tracking-wide">{t.label}</span>
            </button>
          ))}
        </nav>
      </div>
    </IntlProvider>
  );
}

export default App;
