/// Typed wrappers around the Tauri command surface.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppConfig, DeviceInfo, EngineEvent, EngineSnapshot, HrSample } from "./types";

export const api = {
  snapshot: () => invoke<EngineSnapshot>("snapshot"),
  scan: (timeout_ms = 5000, filter_hr = true) =>
    invoke<DeviceInfo[]>("scan", { timeoutMs: timeout_ms, filterHr: filter_hr }),
  connect: (address: string) => invoke<void>("connect", { address }),
  disconnect: () => invoke<void>("disconnect"),
  resetSession: () => invoke<void>("reset_session"),
  history: (limit = 300) => invoke<{ samples: HrSample[] }>("history", { limit }),
  getConfig: () => invoke<AppConfig>("get_config"),
  saveConfig: (config: AppConfig) => invoke<void>("save_config", { config }),
  configPaths: () => invoke<{ config: string; data: string }>("config_paths"),
  saveDevice: (address: string) => invoke<void>("save_device", { address }),
  getLogs: (limit = 200) => invoke<string[]>("get_logs", { limit }),
  getOverlayHtml: () => invoke<string | null>("get_overlay_html"),
  saveOverlayHtml: (html: string) => invoke<void>("save_overlay_html", { html }),
  resetOverlayHtml: () => invoke<void>("reset_overlay_html"),
  getDefaultOverlayHtml: () => invoke<string>("get_default_overlay_html"),
};

export function onEngineEvent(handler: (e: EngineEvent) => void): Promise<UnlistenFn> {
  return listen<EngineEvent>("heartkick://event", (e) => handler(e.payload));
}
