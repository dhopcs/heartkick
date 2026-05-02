/// Stat tiles + interactive BPM chart with hover tooltip and time-range filter.

import { useEffect, useMemo, useRef, useState } from "preact/hooks";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";
import type { EngineSnapshot, HrSample } from "../types";
import { api } from "../api";

interface Props {
  snapshot: EngineSnapshot | null;
  recent: HrSample[];
}

function fmtDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  return h > 0
    ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`
    : `${m}:${String(s).padStart(2, "0")}`;
}

function Tile({ label, value, accent }: { label: string; value: string; accent?: boolean }) {
  return (
    <div class="rounded-xl bg-white/5 px-4 py-3">
      <div class="text-xs uppercase tracking-wider text-white/50">{label}</div>
      <div class={`mt-1 text-2xl font-semibold tabular-nums ${accent ? "text-rose-400" : ""}`}>
        {value}
      </div>
    </div>
  );
}

type RelRange = number | null | "session";

const PRESETS: { label: string; secs: RelRange }[] = [
  { label: "Session", secs: "session" },
  { label: "1m", secs: 60 },
  { label: "5m", secs: 300 },
  { label: "15m", secs: 900 },
  { label: "1h", secs: 3600 },
  { label: "All", secs: null },
];

/** RMSSD (ms) from all RR intervals within the provided samples. */
function computeRmssd(samples: HrSample[]): number | null {
  const rrs: number[] = [];
  for (const s of samples) for (const v of s.rr_intervals_ms) rrs.push(v);
  if (rrs.length < 2) return null;
  let sumSq = 0;
  for (let i = 1; i < rrs.length; i++) {
    const d = rrs[i] - rrs[i - 1];
    sumSq += d * d;
  }
  return Math.sqrt(sumSq / (rrs.length - 1));
}

/** Format an abs-range timestamp pair into a compact human-readable label. */
function fmtAbsLabel(from: number, to: number): string {
  const fDate = new Date(from);
  const tDate = new Date(to);
  const sameDay =
    fDate.getFullYear() === tDate.getFullYear() &&
    fDate.getMonth() === tDate.getMonth() &&
    fDate.getDate() === tDate.getDate();
  const time = (d: Date) => d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const dateTime = (d: Date) =>
    d.toLocaleDateString([], { month: "short", day: "numeric" }) + " " + time(d);
  return sameDay ? `${time(fDate)} – ${time(tDate)}` : `${dateTime(fDate)} – ${dateTime(tDate)}`;
}

/** Parse a loosely-typed "YYYY-MM-DD HH:MM" string into a timestamp, or NaN. */
function parseDt(s: string): number {
  return new Date(s.trim().replace(" ", "T")).getTime();
}

/** Simple date+time text field: accepts "YYYY-MM-DD HH:MM". */
function DateTimeField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  const valid = value === "" || !isNaN(parseDt(value));
  return (
    <div class="flex flex-col gap-1">
      <label class="text-[10px] uppercase tracking-wider text-white/40">{label}</label>
      <input
        type="text"
        value={value}
        placeholder="YYYY-MM-DD HH:MM"
        onInput={(e) => onChange((e.target as HTMLInputElement).value)}
        class={`w-full rounded-lg border bg-white/5 px-2.5 py-1.5 font-mono text-xs text-white/80 outline-none transition-colors focus:bg-white/8 ${
          valid ? "border-white/10 focus:border-white/25" : "border-rose-500/60"
        }`}
      />
    </div>
  );
}

export function MetricsPanel({ snapshot, recent }: Props) {
  const [relRange, setRelRange] = useState<RelRange>("session");
  const [absRange, setAbsRange] = useState<{ from: number; to: number } | null>(null);
  const [absFrom, setAbsFrom] = useState("");
  const [absTo, setAbsTo] = useState("");

  const [showPicker, setShowPicker] = useState(false);
  const [resetConfirm, setResetConfirm] = useState(false);
  const pickerRef = useRef<HTMLDivElement>(null);

  // Close picker on outside click
  useEffect(() => {
    if (!showPicker) return;
    function onDown(e: MouseEvent) {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) {
        setShowPicker(false);
      }
    }
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [showPicker]);

  const session = snapshot?.session;
  const sessionStartedAt = session?.started_at ?? null;
  const startedAt = sessionStartedAt ? new Date(sessionStartedAt).getTime() : null;
  const lastAt = session?.last_at ? new Date(session.last_at).getTime() : null;
  const durationSecs = startedAt && lastAt ? Math.floor((lastAt - startedAt) / 1000) : 0;

  // Filter samples to selected time range (relative, absolute, session, or all)
  const filtered = useMemo<HrSample[]>(() => {
    if (absRange) {
      return recent.filter((s) => {
        const t = new Date(s.timestamp).getTime();
        return t >= absRange.from && t <= absRange.to;
      });
    }
    if (relRange === "session") {
      return startedAt
        ? recent.filter((s) => new Date(s.timestamp).getTime() >= startedAt)
        : recent;
    }
    if (relRange !== null) {
      const cutoff = Date.now() - relRange * 1000;
      return recent.filter((s) => new Date(s.timestamp).getTime() >= cutoff);
    }
    return recent;
  }, [recent, relRange, absRange, startedAt]);

  // For the session view use the authoritative snapshot stats directly.
  // For all other views compute from the filtered window.
  const useSessionStats = relRange === "session" && !absRange;

  // Single-pass min/max/avg + RMSSD from filtered window (used when not session view)
  const { rangeMin, rangeMax, rangeAvg, rangeRmssd } = useMemo(() => {
    if (filtered.length === 0)
      return { rangeMin: null, rangeMax: null, rangeAvg: null, rangeRmssd: null };
    let min = filtered[0].bpm,
      max = filtered[0].bpm,
      sum = 0;
    for (const s of filtered) {
      if (s.bpm < min) min = s.bpm;
      if (s.bpm > max) max = s.bpm;
      sum += s.bpm;
    }
    return {
      rangeMin: min,
      rangeMax: max,
      rangeAvg: sum / filtered.length,
      rangeRmssd: computeRmssd(filtered),
    };
  }, [filtered]);

  const displayMin = useSessionStats ? (session?.min_bpm ?? null) : rangeMin;
  const displayMax = useSessionStats ? (session?.max_bpm ?? null) : rangeMax;
  const displayAvg = useSessionStats ? (session?.avg_bpm ?? null) : rangeAvg;
  const rmssd = useSessionStats ? (snapshot?.rmssd ?? null) : rangeRmssd;

  const rr = snapshot?.last_sample?.rr_intervals_ms ?? [];
  const lastRr = rr.length > 0 ? rr[rr.length - 1] : null;

  const rangeLabel = absRange
    ? fmtAbsLabel(absRange.from, absRange.to)
    : (PRESETS.find((p) => p.secs === relRange)?.label ?? "Session");

  function applyAbsRange() {
    // Accept "YYYY-MM-DD HH:MM" or "YYYY-MM-DDTHH:MM"
    const parse = (s: string) => new Date(s.trim().replace(" ", "T")).getTime();
    const from = parse(absFrom);
    const to = parse(absTo);
    if (isNaN(from) || isNaN(to) || from >= to) return;
    setAbsRange({ from, to });
    setRelRange(null);
    setShowPicker(false);
  }

  function selectPreset(secs: RelRange) {
    setRelRange(secs);
    setAbsRange(null);
    setAbsFrom("");
    setAbsTo("");
    setShowPicker(false);
  }

  function openPicker() {
    const fmt = (ms: number) => {
      const d = new Date(ms);
      const pad = (n: number) => String(n).padStart(2, "0");
      return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
    };
    if (absRange) {
      setAbsFrom(fmt(absRange.from));
      setAbsTo(fmt(absRange.to));
    } else if (!absFrom || !absTo) {
      const now = Date.now();
      setAbsFrom(fmt(now - 3600000));
      setAbsTo(fmt(now));
    }
    setShowPicker((v) => !v);
  }

  return (
    <div class="space-y-4">
      <div class="flex items-center justify-between">
        <h2 class="text-lg font-semibold">Metrics</h2>
        <div class="flex items-center gap-2">
          {/* Time range picker */}
          <div class="relative" ref={pickerRef}>
            <button
              class="flex items-center gap-1 rounded-lg border border-white/10 bg-white/5 px-3 py-1 text-sm text-white/80 transition-colors hover:bg-white/10"
              onClick={openPicker}
            >
              {rangeLabel}
              <svg class="h-3 w-3 opacity-50" viewBox="0 0 10 6" fill="currentColor">
                <path d="M0 0l5 6 5-6z" />
              </svg>
            </button>
            {showPicker && (
              <div class="absolute right-0 top-full z-20 mt-1 w-64 rounded-xl border border-white/10 bg-neutral-900 p-3 shadow-xl">
                {/* Relative presets */}
                <div class="flex gap-1">
                  {PRESETS.map(({ label, secs }) => (
                    <button
                      key={label}
                      class={`flex-1 rounded-md py-1 text-xs font-medium transition-colors ${
                        !absRange && relRange === secs
                          ? "bg-white/20 text-white"
                          : "text-white/50 hover:bg-white/10 hover:text-white"
                      }`}
                      onClick={() => selectPreset(secs)}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                {/* Custom absolute range */}
                <div class="mt-3 border-t border-white/10 pt-3 flex flex-col gap-2">
                  <DateTimeField label="From" value={absFrom} onChange={setAbsFrom} />
                  <DateTimeField label="To" value={absTo} onChange={setAbsTo} />
                  <button
                    class="mt-1 w-full rounded-lg bg-rose-500/80 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-rose-500 disabled:opacity-30"
                    disabled={!absFrom || !absTo}
                    onClick={applyAbsRange}
                  >
                    Apply range
                  </button>
                </div>
              </div>
            )}
          </div>

          <button
            class="rounded-lg bg-white/10 px-3 py-1 text-sm transition-colors hover:bg-white/15"
            onClick={() => setResetConfirm(true)}
          >
            Reset
          </button>
        </div>
      </div>

      <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
        <Tile label="Current" value={String(snapshot?.last_sample?.bpm ?? "-")} accent />
        <Tile label="Min" value={displayMin != null ? String(displayMin) : "-"} />
        <Tile label="Max" value={displayMax != null ? String(displayMax) : "-"} />
        <Tile label="Avg" value={displayAvg != null ? displayAvg.toFixed(1) : "-"} />
        <Tile label="RMSSD" value={rmssd != null ? `${(rmssd as number).toFixed(0)} ms` : "-"} />
        <Tile label="Duration" value={fmtDuration(durationSecs)} />
      </div>

      <div class="grid grid-cols-2 gap-3">
        <Tile label="Last RR" value={lastRr != null ? `${lastRr} ms` : "-"} />
        <Tile label="Samples" value={String(filtered.length)} />
      </div>

      <HrChart
        samples={filtered}
        rangeLabel={rangeLabel}
        onSelectRange={(from, to) => {
          setAbsRange({ from, to });
          setRelRange(null);
          // Clear stale inputs so openPicker always re-syncs from absRange
          setAbsFrom("");
          setAbsTo("");
        }}
      />

      {/* Reset confirmation modal */}
      {resetConfirm && (
        <div
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
          onClick={() => setResetConfirm(false)}
        >
          <div
            class="w-72 rounded-2xl border border-white/10 bg-neutral-900 p-5 shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 class="text-base font-semibold">Reset session?</h3>
            <p class="mt-1 text-sm text-white/50">
              This will clear all current session data. This cannot be undone.
            </p>
            <div class="mt-4 flex justify-end gap-2">
              <button
                class="rounded-lg bg-white/10 px-4 py-1.5 text-sm transition-colors hover:bg-white/15"
                onClick={() => setResetConfirm(false)}
              >
                Cancel
              </button>
              <button
                class="rounded-lg bg-rose-500 px-4 py-1.5 text-sm font-medium transition-colors hover:bg-rose-400"
                onClick={() => {
                  api.resetSession();
                  setResetConfirm(false);
                }}
              >
                Reset
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function HrChart({
  samples,
  rangeLabel,
  onSelectRange,
}: {
  samples: HrSample[];
  rangeLabel: string;
  onSelectRange: (from: number, to: number) => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const plotRef = useRef<uPlot | null>(null);
  const roRef = useRef<ResizeObserver | null>(null);
  // Keep a ref so the hook closure always calls the latest callback
  // without needing to recreate the plot when the parent re-renders.
  const onSelectRef = useRef(onSelectRange);
  onSelectRef.current = onSelectRange;

  const hasData = samples.length >= 2;

  // Create or update the plot whenever samples change.
  // The plot is created lazily on the first render that has enough data.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    if (!hasData) {
      plotRef.current?.destroy();
      plotRef.current = null;
      roRef.current?.disconnect();
      roRef.current = null;
      return;
    }

    // Build xs/ys inserting a null into the y-series wherever consecutive
    // samples are more than 10 s apart so the chart shows an empty gap
    // instead of a long bridging line across disconnects.
    const GAP_S = 10;
    const xs: number[] = [];
    const ys: (number | null)[] = [];
    for (let i = 0; i < samples.length; i++) {
      const t = new Date(samples[i].timestamp).getTime() / 1000;
      if (i > 0 && t - xs[xs.length - 1] > GAP_S) {
        xs.push((xs[xs.length - 1] + t) / 2);
        ys.push(null);
      }
      xs.push(t);
      ys.push(samples[i].bpm);
    }

    if (plotRef.current) {
      plotRef.current.setData([xs, ys]);
      return;
    }

    // Resolve the CSS variable to a real colour string so canvas can use it.
    const pulseColor = getComputedStyle(el).getPropertyValue("--color-pulse").trim() || "#ec4899";

    const opts: uPlot.Options = {
      width: el.clientWidth || 600,
      height: 168,
      padding: [8, 4, 0, 4],
      cursor: { show: true, points: { size: 5, fill: pulseColor } },
      select: { show: true, left: 0, top: 0, width: 0, height: 0 },
      legend: { show: false },
      hooks: {
        setSelect: [
          (u) => {
            if (u.select.width > 0) {
              const from = u.posToVal(u.select.left, "x") * 1000;
              const to = u.posToVal(u.select.left + u.select.width, "x") * 1000;
              onSelectRef.current(from, to);
              // Clear the selection overlay so it doesn't linger.
              u.setSelect({ left: 0, top: 0, width: 0, height: 0 }, false);
            }
          },
        ],
      },
      axes: [
        {
          stroke: "rgba(255,255,255,0.35)",
          ticks: { show: false },
          grid: { show: false },
          border: { show: false },
          values: (_u: uPlot, splits: number[]) =>
            splits.map((s) =>
              new Date(s * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
            ),
        },
        {
          stroke: "rgba(255,255,255,0.35)",
          ticks: { show: false },
          grid: { show: false },
          border: { show: false },
          size: 36,
        },
      ],
      series: [
        {},
        {
          stroke: pulseColor,
          width: 2.5,
          points: { show: false },
          spanGaps: false,
        },
      ],
    };

    const u = new uPlot(opts, [xs, ys], el);
    plotRef.current = u;

    function resize() {
      u.setSize({ width: el!.clientWidth, height: 168 });
    }
    const ro = new ResizeObserver(resize);
    ro.observe(el);
    roRef.current = ro;

    return () => {
      ro.disconnect();
      u.destroy();
      plotRef.current = null;
      roRef.current = null;
    };
  }, [samples, hasData]);

  return (
    <div class="rounded-xl bg-white/5 p-4 space-y-3">
      <div class="flex items-center justify-between">
        <h3 class="text-sm font-semibold uppercase tracking-wider text-white/60">Heart Rate</h3>
        <span class="text-xs text-white/30">{rangeLabel}</span>
      </div>
      <div ref={containerRef} class="[&_.uplot]:!w-full select-none">
        {!hasData && (
          <div class="grid h-36 place-items-center text-sm text-white/30">Awaiting data…</div>
        )}
      </div>
    </div>
  );
}
