import { defineStore } from "pinia";
import { onScopeDispose, ref } from "vue";
import { ApiError } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import type { DeviceStart, GithubRepo, GithubStatus, GithubUser } from "@/core/types";
import { errorMessage } from "@workbench/utils";

export const useGithubStore = defineStore("github", () => {
  const daemon = useDaemon();
  const configured = ref(false);
  const deviceFlow = ref(false);
  const source = ref<GithubStatus["source"]>(null);
  const user = ref<GithubUser | null>(null);
  /** The code being shown while the user approves it on github.com. */
  const device = ref<DeviceStart | null>(null);
  const error = ref<string | null>(null);
  const repos = ref<GithubRepo[]>([]);
  const reposLoading = ref(false);
  let timer: ReturnType<typeof setTimeout> | null = null;
  /** Bumped on every cancel/sign-in so results from superseded flows are ignored. */
  let flow = 0;
  onScopeDispose(stopTimer);

  async function load() {
    const s = await daemon.api.githubStatus();
    configured.value = s.configured;
    deviceFlow.value = s.deviceFlow;
    source.value = s.source;
    user.value = s.user;
  }

  function stopTimer() {
    if (timer) clearTimeout(timer);
    timer = null;
  }

  function schedule(id: number, seconds: number) {
    stopTimer();
    if (id !== flow) return;
    timer = setTimeout(() => void poll(id, seconds), seconds * 1000);
  }

  async function signIn() {
    cancel();
    const id = flow;
    try {
      const start = await daemon.api.githubDeviceStart();
      if (id !== flow) return;
      device.value = start;
      schedule(id, start.interval);
    } catch (e) {
      if (id === flow) error.value = errorMessage(e);
    }
  }

  async function poll(id: number, interval: number) {
    timer = null;
    if (id !== flow || !device.value) return;
    try {
      const r = await daemon.api.githubDevicePoll();
      if (id !== flow || !device.value) return; // cancelled or superseded while in flight
      switch (r.status) {
        case "pending":
          schedule(id, interval);
          break;
        case "slowDown":
          schedule(id, r.interval);
          break;
        case "signedIn":
          user.value = r.user;
          source.value = "oauth";
          device.value = null;
          break;
        case "expired":
          error.value = "The sign-in code expired. Try again.";
          device.value = null;
          break;
        case "denied":
          error.value = "Sign-in was cancelled on GitHub.";
          device.value = null;
          break;
      }
    } catch (e) {
      if (id !== flow) return;
      error.value = errorMessage(e);
      device.value = null;
    }
  }

  function cancel() {
    flow++;
    stopTimer();
    device.value = null;
    error.value = null;
  }

  async function signOut() {
    await daemon.api.githubSignOut();
    user.value = null;
    source.value = null;
    repos.value = [];
    void load().catch(() => {}); // a gh login may still apply
  }

  async function loadRepos() {
    reposLoading.value = true;
    try {
      repos.value = await daemon.api.githubRepos();
    } catch (e) {
      // The daemon signs the user out when GitHub rejects the token; mirror that here.
      if (user.value && e instanceof ApiError && e.code === "GITHUB_ERR" && e.status === 400) {
        user.value = null;
        repos.value = [];
        void load().catch(() => {}); // resync with the daemon's real state
      }
      throw e;
    } finally {
      reposLoading.value = false;
    }
  }

  return { configured, deviceFlow, source, user, device, error, repos, reposLoading, load, signIn, cancel, signOut, loadRepos };
});
