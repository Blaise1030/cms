// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { ApiError } from "@/core/api";
import { useGithubStore } from "./store";

const ada = { login: "ada", id: 42, name: "Ada", avatarUrl: "a.png" };

function setup(api: Record<string, unknown>) {
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  return useGithubStore();
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("github store", () => {
  it("loads status", async () => {
    const gh = setup({ githubStatus: vi.fn(async () => ({ configured: true, user: ada })) });
    await gh.load();
    expect(gh.configured).toBe(true);
    expect(gh.user).toEqual(ada);
  });

  it("polls the device flow at the given interval until signed in", async () => {
    const poll = vi
      .fn()
      .mockResolvedValueOnce({ status: "pending" })
      .mockResolvedValueOnce({ status: "slowDown", interval: 10 })
      .mockResolvedValueOnce({ status: "signedIn", user: ada });
    const gh = setup({
      githubDeviceStart: vi.fn(async () => ({
        userCode: "ABCD-1234", verificationUri: "https://github.com/login/device",
        expiresIn: 900, interval: 5,
      })),
      githubDevicePoll: poll,
    });
    await gh.signIn();
    expect(gh.device?.userCode).toBe("ABCD-1234");

    await vi.advanceTimersByTimeAsync(4999);
    expect(poll).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1); // 5s: pending
    expect(poll).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(5000); // 10s: slowDown → next in 10s
    expect(poll).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(9999);
    expect(poll).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1);
    await flushPromises();
    expect(poll).toHaveBeenCalledTimes(3);
    expect(gh.user).toEqual(ada);
    expect(gh.device).toBeNull();
  });

  it("reports an expired code and stops polling", async () => {
    const poll = vi.fn(async () => ({ status: "expired" }));
    const gh = setup({
      githubDeviceStart: vi.fn(async () => ({ userCode: "X", verificationUri: "u", expiresIn: 1, interval: 1 })),
      githubDevicePoll: poll,
    });
    await gh.signIn();
    await vi.advanceTimersByTimeAsync(5000);
    expect(poll).toHaveBeenCalledTimes(1);
    expect(gh.device).toBeNull();
    expect(gh.error).toMatch(/expired/);
  });

  it("cancel stops polling", async () => {
    const poll = vi.fn(async () => ({ status: "pending" }));
    const gh = setup({
      githubDeviceStart: vi.fn(async () => ({ userCode: "X", verificationUri: "u", expiresIn: 900, interval: 1 })),
      githubDevicePoll: poll,
    });
    await gh.signIn();
    gh.cancel();
    await vi.advanceTimersByTimeAsync(5000);
    expect(poll).not.toHaveBeenCalled();
  });

  it("signs out", async () => {
    const signOut = vi.fn(async () => ({ signedOut: true }));
    const gh = setup({ githubSignOut: signOut });
    gh.user = ada;
    await gh.signOut();
    expect(signOut).toHaveBeenCalled();
    expect(gh.user).toBeNull();
  });

  it("clears the user when the daemon reports the session expired", async () => {
    const gh = setup({
      githubRepos: vi.fn(async () => {
        throw new ApiError("GITHUB_ERR", "your GitHub session expired — sign in again", 400);
      }),
    });
    gh.user = ada;
    await expect(gh.loadRepos()).rejects.toThrow(/expired/);
    expect(gh.user).toBeNull();
    expect(gh.repos).toEqual([]);
    expect(gh.reposLoading).toBe(false);
  });

  it("keeps the user on other repo-list failures", async () => {
    const gh = setup({
      githubRepos: vi.fn(async () => {
        throw new ApiError("GITHUB_ERR", "GitHub is down", 502);
      }),
    });
    gh.user = ada;
    await expect(gh.loadRepos()).rejects.toThrow(/down/);
    expect(gh.user).toEqual(ada);
  });
});
