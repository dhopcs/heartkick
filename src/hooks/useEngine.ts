/// Live engine state hook. Hydrates from snapshot, then keeps in sync with
/// streamed engine events.

import { useEffect, useRef, useState } from "preact/hooks";
import { api, onEngineEvent } from "../api";
import type { EngineSnapshot, HrSample } from "../types";

export function useEngine() {
  const [snapshot, setSnapshot] = useState<EngineSnapshot | null>(null);
  const [recent, setRecent] = useState<HrSample[]>([]);
  // Ref so sample handler can read latest recent without stale closure.
  const recentRef = useRef<HrSample[]>([]);

  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | null = null;

    (async () => {
      const [snap, hist] = await Promise.all([api.snapshot(), api.history(120)]);
      if (!alive) return;
      recentRef.current = hist.samples;
      setSnapshot(snap);
      setRecent(hist.samples);

      unlisten = await onEngineEvent((evt) => {
        if (evt.type === "sample") {
          const sample: HrSample = {
            bpm: evt.bpm,
            rr_intervals_ms: evt.rr_intervals_ms,
            timestamp: evt.timestamp,
          };
          const prev = recentRef.current;
          const next = prev.length >= 600 ? [...prev.slice(1), sample] : [...prev, sample];
          recentRef.current = next;
          setRecent(next);

          // Use the authoritative rmssd and session values from the event -
          // the backend already maintains both so no need to recompute here.
          setSnapshot((s) =>
            s ? { ...s, last_sample: sample, rmssd: evt.rmssd, session: evt.session } : s,
          );
        } else if (evt.type === "state") {
          setSnapshot((prev) =>
            prev ? { ...prev, state: evt.state, device_address: evt.device } : prev,
          );
        } else if (evt.type === "session_reset") {
          recentRef.current = [];
          setRecent([]);
          api.snapshot().then((s) => alive && setSnapshot(s));
        }
      });
    })();

    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  return { snapshot, recent };
}
