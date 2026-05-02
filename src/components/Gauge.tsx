/// Big circular gauge with a pulsing heart in the centre.

import type { EngineSnapshot } from "../types";

interface Props {
  snapshot: EngineSnapshot | null;
}

export function Gauge({ snapshot }: Props) {
  const bpm = snapshot?.last_sample?.bpm ?? 0;
  const max = 200;
  const pct = Math.min(1, Math.max(0, bpm / max));
  const radius = 110;
  const circ = 2 * Math.PI * radius;
  const offset = circ * (1 - pct);

  // A short fixed duration for the single-fire animation; the key change
  // (driven by sample timestamp) restarts it on each incoming beat.
  const pulseDuration = "0.35s";
  // key changes every new sample → Preact remounts the element → animation
  // restarts from the beginning, playing exactly once per beat.
  const pulseKey = snapshot?.last_sample?.timestamp ?? "idle";

  return (
    <div class="relative flex items-center justify-center select-none">
      <svg width="280" height="280" viewBox="-140 -140 280 280" class="-rotate-90">
        <circle r={radius} fill="none" stroke="rgba(255,255,255,0.08)" stroke-width="14" />
        <circle
          r={radius}
          fill="none"
          stroke="var(--color-pulse)"
          stroke-width="14"
          stroke-linecap="round"
          stroke-dasharray={circ}
          stroke-dashoffset={offset}
          style={{ transition: "stroke-dashoffset 400ms ease" }}
        />
      </svg>
      <div class="absolute inset-0 flex flex-col items-center justify-center gap-2">
        <svg
          key={pulseKey}
          class={`heartkick-pulse shrink-0 ${bpm > 0 ? "" : "opacity-30"}`}
          style={{
            ["--pulse-duration" as string]: pulseDuration,
            color: "var(--color-pulse)",
          }}
          width="64"
          height="58"
          viewBox="0 0 100 90"
          fill="currentColor"
          aria-hidden
        >
          <path d="M50 85 C50 85 5 55 5 28 C5 12 18 0 35 0 C43 0 50 6 50 6 C50 6 57 0 65 0 C82 0 95 12 95 28 C95 55 50 85 50 85Z" />
        </svg>
        <div class="text-5xl font-semibold tabular-nums leading-none">{bpm > 0 ? bpm : "-"}</div>
        <div class="text-sm uppercase tracking-widest text-white/50">bpm</div>
      </div>
    </div>
  );
}
