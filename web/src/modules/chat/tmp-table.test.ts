// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import type { UIMessage } from "ai";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";

const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("vue-sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import ChatMessage from "./ChatMessage.vue";

describe("tmp table", () => {
  it("renders a table", async () => {
    h.daemon = createFakeDaemon({});
    const md = [
      "**HTTP (project-scoped, hard cutover)**",
      "",
      "| Method | Path | Role |",
      "|---|---|---|",
      "| GET | `/api/chat/{project}` | List chats |",
      "| POST | `/api/chat/{project}` | Create chat |",
    ].join("\n");
    const message = { id: "m1", role: "assistant", parts: [{ type: "text", text: md }] } as UIMessage;
    const w = mount(ChatMessage, { props: { projectId: "p", message }, attachTo: document.body });
    await flushPromises();
    console.log(w.html());
    expect(document.body.querySelector("table")).not.toBeNull();
    w.unmount();
  });
});
