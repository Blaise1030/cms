import { defineStore } from "pinia";
import { ref } from "vue";
import { ApiError } from "@/core/api";
import { useDaemon } from "@/core/daemon";
import type { DeviceStart, GithubRepo, GithubUser } from "@/core/types";
import { errorMessage } from "@/lib/errors";

export const useGithubStore = defineStore("github", () => {
  const daemon = useDaemon();
  const configured = ref(false);
  const user = ref<GithubUser | null>(null);
  /** The code being shown while the user approves it on github.com. */
  const device = ref<DeviceStart | null>(null);
  const error = ref<string | null>(null);
  const repos = ref<GithubRepo[]>([]);
  const reposLoading = ref(false);
  let timer: ReturnType<typeof setTimeout> | null = null;

  async function load() {
    const s = await daemon.api.githubStatus();
    configured.value = s.configured;
    user.value = s.user;
  }

  function stopTimer() {
    if (timer) clearTimeout(timer);
    timer = null;
  }

  function schedule(seconds: number) {
    stopTimer();
    timer = setTimeout(() => void poll(seconds), seconds * 1000);
  }

  async function signIn() {
    cancel();
    error.value = null;
    try {
      device.value = await daemon.api.githubDeviceStart();
      schedule(device.value.interval);
    } catch (e) {
      error.value = errorMessage(e);
    }
  }

  async function poll(interval: number) {
    timer = null;
    if (!device.value) return;
    try {
      const r = await daemon.api.githubDevicePoll();
      if (!device.value) return; // cancelled while the request was in flight
      switch (r.status) {
        case "pending":
          schedule(interval);
          break;
        case "slowDown":
          schedule(r.interval);
          break;
        case "signedIn":
          user.value = r.user;
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
      error.value = errorMessage(e);
      device.value = null;
    }
  }

  function cancel() {
    stopTimer();
    device.value = null;
  }

  async function signOut() {
    await daemon.api.githubSignOut();
    user.value = null;
    repos.value = [];
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
      }
      throw e;
    } finally {
      reposLoading.value = false;
    }
  }

  return { configured, user, device, error, repos, reposLoading, load, signIn, cancel, signOut, loadRepos };
});
