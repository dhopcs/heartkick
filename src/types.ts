/// Types mirrored from the Rust API.

export type ConnectionState = "disconnected" | "scanning" | "connecting" | "connected";

export interface HrSample {
  bpm: number;
  rr_intervals_ms: number[];
  timestamp: string;
}

export interface SessionStats {
  started_at: string | null;
  last_at: string | null;
  samples: number;
  min_bpm: number | null;
  max_bpm: number | null;
  avg_bpm: number | null;
}

export interface EngineSnapshot {
  state: ConnectionState;
  device_address: string | null;
  last_sample: HrSample | null;
  session: SessionStats;
  rmssd: number | null;
  battery: number | null;
}

export type EngineEvent =
  | {
      type: "sample";
      bpm: number;
      rr_intervals_ms: number[];
      timestamp: string;
      rmssd: number | null;
      session: SessionStats;
    }
  | { type: "state"; state: ConnectionState; device: string | null }
  | { type: "session_reset" };

export interface DeviceInfo {
  address: string;
  name: string | null;
  rssi: number | null;
  advertises_hr: boolean;
}

export interface WebhookConfig {
  name: string;
  enabled: boolean;
  method: string;
  url: string;
  headers: Record<string, string>;
  body: string;
  min_interval_ms: number;
}

export interface AppConfig {
  general: { locale: string; log_level: string };
  bluetooth: { device_address: string | null; auto_reconnect: boolean };
  api: {
    http_enabled: boolean;
    http_bind: string;
    socket_enabled: boolean;
    socket_path: string | null;
    api_token: string | null;
  };
  integrations: {
    webhooks: WebhookConfig[];
    prometheus: {
      enabled: boolean;
      bind: string;
      push: { enabled: boolean; url: string; headers: Record<string, string> } | null;
    };
    osc: { enabled: boolean; target: string; address: string };
    overlay: { enabled: boolean; bind: string };
  };
}
