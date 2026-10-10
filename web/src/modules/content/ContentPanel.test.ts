// @vitest-environment happy-dom
import { expect, it, vi } from "vitest";
import { createRouter, createMemoryHistory, RouterView } from "vue-router";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
import ContentPanel from "./ContentPanel.vue";
import { useContentStore } from "./store";
const entry = { collection: "blog", id: "welcome", path: "src/content/blog/en/welcome.md", kind: "markdown", source: "Original", fields: {title: "Welcome"}, body: "Original", branch: "main", hash: "abc" };
async function setup() {
 h.daemon = createFakeDaemon({ listContentCollections: async () => [{name:"blog",kind:"markdown",editable:true,reason:null}], listContentEntries: async () => [entry], contentEntry: async () => entry });
 setActivePinia(createPinia());
 const router = createRouter({history:createMemoryHistory(),routes:[{path:"/",component:ContentPanel,props:{projectId:"p"}},{path:"/other",component:{template:"<p>Other</p>"}}]});
 await router.push("/"); await router.isReady();
 return mount(RouterView, {global:{plugins:[router]}});
}
it("navigates collections and entries and reads content on main", async () => {
 const w = await setup(); await flushPromises();
 await w.get('[data-testid="content-collection"]').trigger("click"); await flushPromises();
 await w.get('[data-testid="content-entry"]').trigger("click"); await flushPromises();
 expect(w.text()).toContain("main"); expect(w.get('[data-testid="content-source"]').text()).toBe("Original");
 w.unmount();
});
it("preserves a dirty draft on filesystem refresh and cancelled navigation", async () => {
 const w = await setup(); await flushPromises(); const store = useContentStore();
 await store.selectCollection("blog"); await store.selectEntry("welcome"); store.draftSource = "Draft";
 h.daemon.emit({type:"fs.change",projectId:"p",path:entry.path,kind:"modify"}); await flushPromises();
 expect(store.draftSource).toBe("Draft"); expect(store.dirty).toBe(true); expect(store.changedOnDisk).toBe(true);
 vi.stubGlobal("confirm", vi.fn(() => false)); await store.selectCollection("blog"); expect(store.entry?.id).toBe("welcome");
 vi.unstubAllGlobals(); w.unmount();
});
it("blocks leaving the Content tab until a dirty draft is discarded", async () => {
 const w = await setup(); await flushPromises(); const store = useContentStore();
 await store.selectCollection("blog"); await store.selectEntry("welcome"); store.draftSource = "Draft";
 vi.stubGlobal("confirm", vi.fn(() => false));
 await w.vm.$router.push("/other");
 expect(w.vm.$route.path).toBe("/"); expect(store.draftSource).toBe("Draft");
 vi.stubGlobal("confirm", vi.fn(() => true));
 await w.vm.$router.push("/other"); expect(w.vm.$route.path).toBe("/other");
 expect(store.dirty).toBe(false); expect(store.draftSource).toBe("Original");
 vi.unstubAllGlobals(); w.unmount();
});
