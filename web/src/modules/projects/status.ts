import type { DevserverState } from "@/core/types";

export type BadgeTone = "default" | "secondary" | "destructive" | "outline";

export const STATUS_LABEL: Record<DevserverState, string> = {
  starting: "Starting",
  ready: "Ready",
  crashed: "Crashed",
};

export const STATUS_TONE: Record<DevserverState, BadgeTone> = {
  starting: "secondary",
  ready: "default",
  crashed: "destructive",
};
