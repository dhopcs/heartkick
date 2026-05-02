import { Gauge } from "../components/Gauge";
import type { EngineSnapshot } from "../types";

interface Props {
  snapshot: EngineSnapshot | null;
  onGoToDevices: () => void;
}

export function HomePage({ snapshot, onGoToDevices }: Props) {
  const isConnected = snapshot?.state === "connected";

  return (
    <div class="space-y-8">
      <div class="flex justify-center pt-4">
        <Gauge snapshot={snapshot} />
      </div>
      {!isConnected && (
        <div class="rounded-xl border border-amber-500/20 bg-amber-500/10 px-4 py-4">
          <div class="text-sm font-semibold text-amber-400">No device connected</div>
          <p class="mt-1 text-sm text-white/50">
            Pair with a heart rate monitor to start getting metrics.{" "}
            <button
              class="text-amber-400 underline underline-offset-2 hover:text-amber-300 transition-colors"
              onClick={onGoToDevices}
            >
              Go to Devices
            </button>
          </p>
        </div>
      )}
    </div>
  );
}
