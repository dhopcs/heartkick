import { MetricsPanel } from "../components/MetricsPanel";
import type { EngineSnapshot, HrSample } from "../types";

interface Props {
  snapshot: EngineSnapshot | null;
  recent: HrSample[];
}

export function MetricsPage({ snapshot, recent }: Props) {
  return <MetricsPanel snapshot={snapshot} recent={recent} />;
}
