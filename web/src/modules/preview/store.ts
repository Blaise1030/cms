import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { useLocalStorage } from "@vueuse/core";
import { useDaemon } from "@/core/daemon";
import type { DevserverState } from "@/core/types";
import { errorMessage } from "@workbench/utils";
import { useAgentOutboxStore } from "@/modules/chat/outbox";
import type { PickedElement } from "./elementPicker";
import { crashPrompt, pageErrorPrompt, type PageError } from "./pageErrors";

export type Device = "desktop" | "tablet" | "mobile";

/** Viewport width per device; `null` fills the panel. */
export const DEVICE_WIDTH: Record<Device, number | null> = {
  desktop: null,
  tablet: 768,
  mobile: 390,
};

export const usePreviewStore = defineStore("preview", () => {
  const daemon = useDaemon();
  const outbox = useAgentOutboxStore();

  const device = ref<Device>("desktop");
  /** Force right-to-left layout in the preview. */
  const rtl = ref(false);
  /** Preview in dark mode (`prefers-color-scheme: dark` and a `dark` class on `<html>`). */
  const dark = useLocalStorage("workbench.preview.dark", false);
  /** Bumping this remounts the iframe. */
  const reloadNonce = ref(0);
  /** Live dev-server state per project, from `devserver.status` events. */
  const status = ref<Record<string, DevserverState>>({});
  const message = ref<Record<string, string | undefined>>({});
  const restarting = ref(false);
  const error = ref<string | null>(null);
  /** Element-picker mode in the preview frame. */
  const picking = ref(false);
  /** Whether each project's agent is working on a request, from `terminal.busy` events. */
  const agentBusy = ref<Record<string, boolean>>({});

  function setDevice(next: Device) {
    device.value = next;
  }

  function toggleRtl() {
    rtl.value = !rtl.value;
  }

  function toggleDark() {
    dark.value = !dark.value;
  }

  function setPicking(on: boolean) {
    picking.value = on;
  }

  function pick(element: PickedElement) {
    outbox.send({ kind: "element", element });
    picking.value = false;
  }

  /** Put a fix request for a page error in the agent's input, to edit before sending. */
  function fixPageError(error: PageError, path: string) {
    outbox.send({ kind: "text", text: pageErrorPrompt(error, path) });
  }

  function fixCrash(projectId: string) {
    outbox.send({ kind: "text", text: crashPrompt(message.value[projectId]) });
  }

  /** Seed the agent's busy state for a project (events keep it current afterwards). */
  async function loadAgentBusy(projectId: string) {
    try {
      const s = await daemon.api.terminalStatus(projectId);
      agentBusy.value[projectId] ??= s.running && s.busy === true;
    } catch {
      // No banner is the safe default.
    }
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

  /**
   * Missed `devserver.status` events: forget the event-derived state so the panel falls back to
   * the (freshly reloaded) project list, and reload the frame in case the server restarted.
   */
  function resync() {
    status.value = {};
    message.value = {};
    const projects = Object.keys(agentBusy.value);
    agentBusy.value = {};
    projects.forEach((id) => void loadAgentBusy(id));
    reload();
  }

  const offs = [
    daemon.onReconnected(resync),
    daemon.on("events.lagged", resync),
    daemon.on("terminal.busy", (e) => {
      agentBusy.value[e.projectId] = e.busy;
    }),
    daemon.on("terminal.exited", (e) => {
      agentBusy.value[e.projectId] = false;
    }),
    daemon.on("devserver.status", (e) => {
      const before = status.value[e.projectId];
      status.value[e.projectId] = e.state;
      message.value[e.projectId] = e.message;
      // The iframe loaded a 502 while the server was booting; reload once it is up.
      if (e.state === "ready" && before !== "ready") reload();
    }),
  ];
  onScopeDispose(() => offs.forEach((off) => off()));

  return {
    device,
    rtl,
    dark,
    reloadNonce,
    status,
    message,
    restarting,
    error,
    picking,
    agentBusy,
    setDevice,
    toggleRtl,
    toggleDark,
    setPicking,
    pick,
    fixPageError,
    fixCrash,
    loadAgentBusy,
    reload,
    seed,
    restart,
  };
});
