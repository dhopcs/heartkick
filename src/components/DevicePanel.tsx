/// Device scan / connect / disconnect controls.

import { useState } from "preact/hooks";
import { api, onEngineEvent } from "../api";
import type { DeviceInfo, EngineSnapshot } from "../types";

interface Props {
  snapshot: EngineSnapshot | null;
  onConnected?: (address: string) => void;
}

export function DevicePanel({ snapshot, onConnected }: Props) {
  const [scanning, setScanning] = useState(false);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [connecting, setConnecting] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);
  const [scanned, setScanned] = useState(false);

  const state = snapshot?.state ?? "disconnected";
  const isConnected = state === "connected";

  async function scan(overrideShowAll?: boolean) {
    const all = overrideShowAll ?? showAll;
    setScanning(true);
    setError(null);
    setDevices([]);

    // Subscribe to device_found events so the list populates as devices appear.
    const unlisten = await onEngineEvent((evt) => {
      if (evt.type !== "device_found") return;
      const d = evt.device;
      if (!all && !d.advertises_hr) return;
      setDevices((prev) =>
        prev.some((x) => x.address === d.address) ? prev : [...prev, d],
      );
    });

    let found: DeviceInfo[] = [];
    try {
      found = await api.scan(8000, !all);
    } catch {
      // On mobile the first scan call may trigger the OS Bluetooth permission
      // dialog, which cancels the ongoing scan. Wait briefly then retry once
      // so the user doesn't have to tap Scan again after granting permission.
      await new Promise<void>((r) => setTimeout(r, 700));
      try {
        found = await api.scan(8000, !all);
      } catch (e) {
        unlisten();
        setError(String(e));
        setScanning(false);
        return;
      }
    }

    unlisten();
    // Replace with the final authoritative list (includes RSSI updates).
    setDevices(found);
    setScanned(true);
    setScanning(false);
  }

  async function connect(addr: string) {
    setConnecting(addr);
    setError(null);
    try {
      await api.connect(addr);
      onConnected?.(addr);
    } catch (e) {
      setError(String(e));
    } finally {
      setConnecting(null);
    }
  }

  if (isConnected) {
    const battery = snapshot?.battery;
    return (
      <div class="flex items-center justify-between rounded-xl border border-emerald-500/20 bg-emerald-500/10 px-4 py-3">
        <div>
          <div class="flex items-center gap-2 text-sm font-medium text-emerald-400">
            Connected
            {battery != null && (
              <span class="text-xs font-normal text-white/50">{battery}% battery</span>
            )}
          </div>
          {snapshot?.device_address && (
            <div class="mt-0.5 font-mono text-xs text-white/40">{snapshot.device_address}</div>
          )}
        </div>
        <button
          class="rounded-lg bg-white/10 px-3 py-1.5 text-sm transition-colors hover:bg-white/15"
          onClick={() => {
            api.disconnect();
            setDevices([]);
          }}
        >
          Disconnect
        </button>
      </div>
    );
  }

  return (
    <div class="space-y-3">
      <div class="flex items-center justify-between gap-2">
        <div class="flex items-center gap-2 text-sm text-white/50">
          <span
            class={`h-2 w-2 rounded-full ${
              state === "connecting"
                ? "bg-amber-400"
                : state === "scanning"
                  ? "animate-pulse bg-blue-400"
                  : "bg-white/20"
            }`}
          />
          <span class="capitalize">{state}</span>
        </div>
        <div class="flex items-center gap-2">
          <button
            class={`rounded-md px-2.5 py-1.5 text-xs transition-colors ${
              showAll ? "bg-white/15 text-white" : "bg-white/5 text-white/40 hover:text-white/70"
            }`}
            onClick={() => setShowAll((v) => !v)}
            title={showAll ? "Showing all BLE devices" : "Showing HR monitors only"}
          >
            {showAll ? "All devices" : "HR only"}
          </button>
          <button
            class="rounded-lg px-4 py-1.5 text-sm font-medium text-white transition-opacity disabled:opacity-40"
            style={{ background: "var(--color-pulse)" }}
            onClick={() => scan()}
            disabled={scanning || state === "connecting"}
          >
            {scanning ? "Scanning…" : "Scan"}
          </button>
        </div>
      </div>

      {error && <div class="rounded-xl bg-red-500/10 px-3 py-2 text-sm text-red-400">{error}</div>}

      {/* Device list — shown while scanning (progressive) and after */}
      {devices.length > 0 && (
        <ul class="divide-y divide-white/5 overflow-hidden rounded-xl bg-white/5">
          {devices.map((d) => (
            <li key={d.address} class="flex items-center justify-between px-4 py-3">
              <div class="min-w-0">
                <div class="truncate text-sm font-medium">{d.name ?? "Unknown device"}</div>
                <div class="mt-0.5 truncate font-mono text-xs text-white/40">
                  {d.address}
                  {d.rssi != null && <span class="ml-2 font-sans">{d.rssi} dBm</span>}
                </div>
              </div>
              <button
                class="ml-3 shrink-0 rounded-md bg-white/10 px-3 py-1.5 text-xs transition-colors hover:bg-white/15 disabled:opacity-40"
                onClick={() => connect(d.address)}
                disabled={connecting === d.address}
              >
                {connecting === d.address ? "Connecting…" : "Connect"}
              </button>
            </li>
          ))}
        </ul>
      )}

      {scanning && devices.length === 0 && (
        <div class="rounded-xl bg-white/5 px-4 py-8 text-center text-sm text-white/40">
          Scanning for {showAll ? "all BLE devices" : "heart rate monitors"}…
        </div>
      )}

      {!scanning && devices.length === 0 && (
        <div class="rounded-xl bg-white/5 px-4 py-8 text-center text-sm text-white/30">
          {!scanned ? (
            <>
              Click <strong class="text-white/50">Scan</strong> to find nearby devices
            </>
          ) : !showAll && !error ? (
            <>
              No heart rate monitors found.{" "}
              <button
                class="text-white/60 underline underline-offset-2 hover:text-white/80 transition-colors"
                onClick={() => {
                  setShowAll(true);
                  scan(true);
                }}
              >
                Search all devices
              </button>
            </>
          ) : null}
        </div>
      )}
    </div>
  );
}
