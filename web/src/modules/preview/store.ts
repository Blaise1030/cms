import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { useDaemon } from "@/core/daemon";
import type { DevserverState } from "@/core/types";
import { errorMessage } from "@/lib/errors";

export type Device = "desktop" | "tablet" | "mobile";

/** Viewport width per device; `null` fills the panel. */
export const DEVICE_WIDTH: Record<Device, number | null> = {
  desktop: null,
  tablet: 768,
  mobile: 390,
};

export const usePreviewStore = defineStore("preview", () => {
  const daemon = useDaemon();

  const device = ref<Device>("desktop");
  /** Bumping this remounts the iframe. */
  const reloadNonce = ref(0);
  /** Live dev-server state per project, from `devserver.status` events. */
  const status = ref<Record<string, DevserverState>>({});
  const message = ref<Record<string, string | undefined>>({});
  const restarting = ref(false);
  const error = ref<string | null>(null);

  function setDevice(next: Device) {
    device.value = next;
  }

  function reload() {
    reloadNonce.value++;
  }

  /** Sync a project's state from the project list (which itself tracks the same events). */
  function seed(projectId: string, state: DevserverState) {
    status.value[projectId] = state;
  }

  async function restart(projectId: string) {
    restarting.value = true;
    error.value = null;
    try {
      await daemon.api.restartDevserver(projectId);
      status.value[projectId] = "starting";
    } catch (e) {
      error.value = errorMessage(e);
    } finally {
      restarting.value = false;
    }
  }

  const offs = [
    daemon.on("devserver.status", (e) => {
      const before = status.value[e.projectId];
      status.value[e.projectId] = e.state;
      message.value[e.projectId] = e.message;
      // The iframe loaded a 502 while the server was booting; reload once it is up.
      if (e.state === "ready" && before !== "ready") reload();
    }),
  ];
  onScopeDispose(() => offs.forEach((off) => off()));

  return { device, reloadNonce, status, message, restarting, error, setDevice, reload, seed, restart };
});
