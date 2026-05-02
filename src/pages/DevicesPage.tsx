import { useState } from "preact/hooks";
import { DevicePanel } from "../components/DevicePanel";
import { api } from "../api";
import type { EngineSnapshot } from "../types";

interface Props {
  snapshot: EngineSnapshot | null;
}

export function DevicesPage({ snapshot }: Props) {
  const [autoConnectAddr, setAutoConnectAddr] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function handleConnected(addr: string) {
    const noAutoConnect = !snapshot?.device_address;
    if (noAutoConnect) {
      setAutoConnectAddr(addr);
    }
  }

  async function confirmAutoConnect() {
    if (!autoConnectAddr) return;
    setSaving(true);
    try {
      await api.saveDevice(autoConnectAddr);
    } finally {
      setSaving(false);
      setAutoConnectAddr(null);
    }
  }

  return (
    <div class="space-y-6">
      <h1 class="text-xl font-semibold">Devices</h1>
      <DevicePanel snapshot={snapshot} onConnected={handleConnected} />

      {autoConnectAddr && (
        <div
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
          onClick={() => setAutoConnectAddr(null)}
        >
          <div
            class="mx-4 w-full max-w-sm rounded-2xl bg-neutral-900 p-6 shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 class="text-base font-semibold">Save auto-connect device?</h3>
            <p class="mt-2 text-sm text-white/60">
              Set <span class="font-mono text-white/80">{autoConnectAddr}</span> as the auto-connect
              device so heartkick reconnects automatically on start.
            </p>
            <div class="mt-5 flex gap-3">
              <button
                class="flex-1 rounded-lg bg-white/10 py-2 text-sm transition-colors hover:bg-white/15"
                onClick={() => setAutoConnectAddr(null)}
              >
                Not now
              </button>
              <button
                class="flex-1 rounded-lg py-2 text-sm font-semibold text-white transition-opacity disabled:opacity-50"
                style={{ background: "var(--color-pulse)" }}
                onClick={confirmAutoConnect}
                disabled={saving}
              >
                {saving ? "Saving…" : "Save"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
