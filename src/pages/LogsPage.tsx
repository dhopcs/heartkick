import { useEffect, useRef, useState } from "preact/hooks";
import { api } from "../api";

export function LogsPage() {
  const [lines, setLines] = useState<string[]>([]);
  const [paused, setPaused] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);
  const pausedRef = useRef(false);

  pausedRef.current = paused;

  useEffect(() => {
    async function fetch() {
      if (!pausedRef.current) {
        const fresh = await api.getLogs(400);
        setLines(fresh);
      }
    }
    fetch();
    const id = setInterval(fetch, 5000);
    return () => clearInterval(id);
  }, []);

  // Auto-scroll to bottom when new lines arrive unless paused
  useEffect(() => {
    if (!paused) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [lines, paused]);

  function levelColor(line: string): string {
    if (/\bERROR\b/.test(line)) return "text-red-400";
    if (/\bWARN\s/.test(line)) return "text-amber-400";
    if (/\bDEBUG\b/.test(line)) return "text-blue-400";
    return "text-white/70";
  }

  return (
    <div class="space-y-3">
      <div class="flex items-center justify-between">
        <h2 class="text-lg font-semibold">Logs</h2>
        <div class="flex gap-2">
          <button
            class={`rounded-lg px-3 py-1.5 text-xs transition-colors ${
              paused
                ? "bg-amber-500/20 text-amber-400"
                : "bg-white/10 text-white/60 hover:bg-white/15"
            }`}
            onClick={() => setPaused((v) => !v)}
          >
            {paused ? "Resume" : "Pause"}
          </button>
          <button
            class="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/15 transition-colors"
            onClick={() => setLines([])}
          >
            Clear
          </button>
        </div>
      </div>

      <div
        class="h-[70vh] overflow-y-auto rounded-xl bg-black/40 p-3 font-mono text-xs leading-relaxed"
        style={{ wordBreak: "break-all" }}
      >
        {lines.length === 0 ? (
          <span class="text-white/30">No log entries yet…</span>
        ) : (
          lines.map((line, i) => (
            <div key={i} class={levelColor(line)}>
              {line}
            </div>
          ))
        )}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
