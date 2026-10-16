// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import UpdateNotice from "./UpdateNotice.vue";
import { UPDATE_EVENT } from "./desktopUpdate";

const applyUrl = "tauri://localhost/update.html?version=1.2.3";

function announce(detail: unknown) {
  window.dispatchEvent(new CustomEvent(UPDATE_EVENT, { detail }));
}

afterEach(() => {
  delete (window as { __WORKBENCH_UPDATE__?: unknown }).__WORKBENCH_UPDATE__;
});

describe("UpdateNotice", () => {
  it("stays hidden until the desktop shell announces an update", async () => {
    const w = mount(UpdateNotice);
    expect(w.find("[data-testid='update-notice']").exists()).toBe(false);

    announce({ version: "1.2.3", applyUrl });
    await w.vm.$nextTick();
    const notice = w.find("[data-testid='update-notice']");
    expect(notice.exists()).toBe(true);
    expect(notice.text()).toContain("Update ready — restart to apply");
    expect(notice.text()).toContain("1.2.3");
    w.unmount();
  });

  it("shows an update announced before it mounted", () => {
    (window as { __WORKBENCH_UPDATE__?: unknown }).__WORKBENCH_UPDATE__ = { version: "1.2.3", applyUrl };
    const w = mount(UpdateNotice);
    expect(w.find("[data-testid='update-notice']").exists()).toBe(true);
    w.unmount();
  });

  it("opens the bundled update page on Restart to update", async () => {
    const navigate = vi.fn();
    const w = mount(UpdateNotice, { props: { navigate } });
    announce({ version: "1.2.3", applyUrl });
    await w.vm.$nextTick();
    await w.find("[data-testid='update-restart']").trigger("click");
    expect(navigate).toHaveBeenCalledWith(applyUrl);
    w.unmount();
  });

  it("can be dismissed", async () => {
    const w = mount(UpdateNotice);
    announce({ version: "1.2.3", applyUrl });
    await w.vm.$nextTick();
    await w.find("[data-testid='update-dismiss']").trigger("click");
    expect(w.find("[data-testid='update-notice']").exists()).toBe(false);
    w.unmount();
  });

  it("ignores announcements that don't point at the bundled update page", async () => {
    const w = mount(UpdateNotice);
    for (const detail of [
      { version: "1.2.3", applyUrl: "https://example.com/update.html" },
      { version: "1.2.3", applyUrl: "javascript:alert(1)" },
      { version: "1.2.3" },
      "1.2.3",
      null,
    ]) {
      announce(detail);
      await w.vm.$nextTick();
      expect(w.find("[data-testid='update-notice']").exists()).toBe(false);
    }
    w.unmount();
  });
});
