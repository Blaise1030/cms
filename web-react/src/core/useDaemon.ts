import { useSyncExternalStore } from "react";
import { getDaemon, type Daemon, type DaemonSnapshot } from "./daemon";

/** Stable daemon API (singleton). Pair with `useDaemonSnapshot` for reactive fields. */
export function useDaemon(): Daemon {
  return getDaemon();
}

/** Reactive connection snapshot — reference-stable until values change. */
export function useDaemonSnapshot(): DaemonSnapshot {
  const daemon = getDaemon();
  return useSyncExternalStore(daemon.subscribe, daemon.getSnapshot, daemon.getSnapshot);
}
