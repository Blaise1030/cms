// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import { useGithubStore } from "./store";
import GithubAccount from "./GithubAccount.vue";

function mountWith(status: unknown, api: Record<string, unknown> = {}) {
  h.daemon = createFakeDaemon({ githubStatus: vi.fn(async () => status), ...api });
  setActivePinia(createPinia());
  return mount(GithubAccount, { attachTo: document.body });
}

enableAutoUnmount(afterEach);

describe("GithubAccount", () => {
  it("renders nothing when sign-in is not configured", async () => {
    const w = mountWith({ configured: false, user: null });
    await flushPromises();
    expect(w.find("[data-testid='github-sign-in']").exists()).toBe(false);
  });

  it("starts the device flow and shows the code", async () => {
    const start = vi.fn(async () => ({
      userCode: "ABCD-1234", verificationUri: "https://github.com/login/device", expiresIn: 900, interval: 5,
    }));
    const w = mountWith({ configured: true, user: null }, { githubDeviceStart: start, githubDevicePoll: vi.fn() });
    await flushPromises();
    await w.find("[data-testid='github-sign-in']").trigger("click");
    await flushPromises();
    expect(start).toHaveBeenCalled();
    expect(document.body.querySelector("[data-testid='device-code']")?.textContent?.trim()).toBe("ABCD-1234");
    useGithubStore().cancel(); // no poll timer may outlive the test
  });

  it("shows the signed-in account", async () => {
    const w = mountWith({ configured: true, user: { login: "ada", id: 1, name: "Ada", avatarUrl: "a.png" } });
    await flushPromises();
    expect(w.find("[data-testid='github-account']").text()).toContain("Ada");
  });
});
