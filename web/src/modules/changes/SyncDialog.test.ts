// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DOMWrapper, flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import type { AutoResolveState, AutoResolveStatus } from "@/core/types";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));

import SyncDialog from "./SyncDialog.vue";
import { useChangesStore } from "./store";
import { useTerminalStore } from "@/modules/terminal/store";

const api = {
  startAutoResolve: vi.fn(async () => ({})),
  answerAutoResolve: vi.fn(async () => ({})),
  cancelAutoResolve: vi.fn(async () => ({})),
  getConflict: vi.fn(async () => ({ id: "s1", files: [], choices: {} })),
};

let wrapper: VueWrapper | null = null;
const body = () => new DOMWrapper(document.body);
const find = (testid: string) => body().find(`[data-testid='${testid}']`);

async function mountWith(status: Partial<AutoResolveStatus> & { state: AutoResolveState }) {
  const store = useChangesStore();
  store.syncSessionId = "s1";
  store.syncStatus = { sessionId: "s1", stage: "", questions: [], summary: null, error: null, ...status };
  wrapper = mount(SyncDialog, { props: { projectId: "site" }, attachTo: document.body });
  await flushPromises();
  return store;
}

async function click(testid: string) {
  await find(testid).trigger("click");
  await flushPromises();
}

beforeEach(() => {
  vi.clearAllMocks();
  h.daemon = createFakeDaemon(api);
  setActivePinia(createPinia());
  useTerminalStore().provider = "claude";
});

afterEach(() => {
  wrapper?.unmount();
  wrapper = null;
  document.body.innerHTML = "";
});

describe("SyncDialog", () => {
  it("shows_stage_while_resolving", async () => {
    await mountWith({ state: "resolving", stage: "Reading both versions" });
    const working = find("sync-working");
    expect(working.exists()).toBe(true);
    expect(working.text()).toContain("Someone else also updated this site.");
    expect(working.text()).toContain("Combining your changes…");
    expect(working.text()).toContain("Reading both versions");
    expect(find("sync-close").exists()).toBe(false);
  });

  it("shows_working_while_loading_and_request_errors_in_any_state", async () => {
    const store = useChangesStore();
    store.syncSessionId = "s1";
    wrapper = mount(SyncDialog, { props: { projectId: "site" }, attachTo: document.body });
    await flushPromises();
    expect(find("sync-working").exists()).toBe(true);
    store.syncStatus = { sessionId: "s1", state: "resolving", stage: "", questions: [], summary: null, error: "Couldn't reach your assistant." };
    await flushPromises();
    expect(body().text()).toContain("Couldn't reach your assistant.");
  });

  it("stop_asks_then_cancels", async () => {
    await mountWith({ state: "resolving" });
    await click("sync-stop");
    expect(body().text()).toContain("Stop combining? Nothing will change.");
    await click("sync-keep-going");
    expect(body().text()).not.toContain("Stop combining?");
    expect(api.cancelAutoResolve).not.toHaveBeenCalled();

    await find("sync-working").trigger("keydown", { key: "Escape" });
    await flushPromises();
    expect(body().text()).toContain("Stop combining? Nothing will change.");
    expect(find("sync-working").exists()).toBe(true);
    await click("sync-confirm-stop");
    expect(api.cancelAutoResolve).toHaveBeenCalledWith("site", "s1");
  });

  it("question_choice_submits_answer", async () => {
    await mountWith({
      state: "needs_input",
      questions: [{ id: "q1", text: "Which headline should stay?", options: ["Mine", "Theirs"] }],
    });
    const question = find("sync-question");
    expect(question.text()).toContain("Which headline should stay?");
    const theirs = question.findAll("[data-slot='questionnaire-choice']").find((c) => c.text() === "Theirs")!;
    await theirs.find("input").setValue(true);
    await click("sync-continue");
    expect(api.answerAutoResolve).toHaveBeenCalledWith("site", "s1", { q1: "Theirs" });
  });

  it("something_else_submits_free_text", async () => {
    await mountWith({
      state: "needs_input",
      questions: [
        { id: "q1", text: "Which headline should stay?", options: ["Mine", "Theirs"] },
        { id: "q2", text: "Which price is right?", options: ["$10", "$12"] },
      ],
    });
    const question = find("sync-question");
    await question.findAll("[data-slot='questionnaire-choice']")[0]!.find("input").setValue(true);
    await click("sync-next");
    const other = question.findAll("[data-testid='sync-other']")[1]!;
    await other.setValue("$11 for now");
    await click("sync-continue");
    expect(api.answerAutoResolve).toHaveBeenCalledWith("site", "s1", { q1: "Mine", q2: "$11 for now" });
  });

  it("done_shows_summary", async () => {
    const store = await mountWith({ state: "done", summary: "Kept your headline and their new price." });
    const done = find("sync-done");
    expect(done.text()).toContain("Done. Your changes and the latest site are combined.");
    expect(done.text()).toContain("Kept your headline and their new price.");
    await click("sync-close");
    expect(store.syncSessionId).toBeNull();
  });

  it("failed_shows_reason_and_retry", async () => {
    await mountWith({ state: "failed", error: "The assistant stopped early." });
    const failed = find("sync-failed");
    expect(failed.text()).toContain("Nothing changed. Your draft is safe.");
    expect(failed.text()).toContain("The assistant stopped early.");
    await click("sync-retry");
    expect(api.startAutoResolve).toHaveBeenCalledWith("site", "s1", "claude");
  });

  it("advanced_opens_conflict_view", async () => {
    const store = await mountWith({ state: "idle" });
    expect(find("sync-failed").exists()).toBe(true);
    await click("sync-advanced");
    expect(api.getConflict).toHaveBeenCalledWith("site", "s1");
    expect(store.selectedConflictId).toBe("s1");
    expect(store.syncSessionId).toBeNull();
  });

  it("copy_never_mentions_git_words", async () => {
    const states: AutoResolveStatus[] = [
      { sessionId: "s1", state: "preparing", stage: "", questions: [], summary: null, error: null },
      { sessionId: "s1", state: "resolving", stage: "", questions: [], summary: null, error: null },
      { sessionId: "s1", state: "verifying", stage: "", questions: [], summary: null, error: null },
      { sessionId: "s1", state: "needs_input", stage: "", questions: [{ id: "q1", text: "Pick one", options: ["A", "B"] }], summary: null, error: null },
      { sessionId: "s1", state: "done", stage: "", questions: [], summary: null, error: null },
      { sessionId: "s1", state: "failed", stage: "", questions: [], summary: null, error: null },
      { sessionId: "s1", state: "idle", stage: "", questions: [], summary: null, error: null },
    ];
    for (const status of states) {
      await mountWith(status);
      const text = body().text();
      expect(text.length).toBeGreaterThan(0);
      expect(text).not.toMatch(/conflict|merge|branch|\bmain\b/i);
      if (status.state !== "done" && status.state !== "failed" && status.state !== "idle") {
        await click("sync-stop");
        expect(body().text()).not.toMatch(/conflict|merge|branch|\bmain\b/i);
      }
      wrapper?.unmount();
      document.body.innerHTML = "";
    }
  });
});
