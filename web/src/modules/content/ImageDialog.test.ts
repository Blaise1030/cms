// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RouterView } from "vue-router";
import { contentRouter } from "@/test/contentRouter";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { EditorView } from "@codemirror/view";
import { createFakeDaemon, type FakeDaemon } from "@/test/fakeDaemon";
import { ApiError } from "@/core/api";
const h = vi.hoisted(() => ({ daemon: null as unknown as FakeDaemon }));
vi.mock("@/core/daemon", () => ({ useDaemon: () => h.daemon }));
vi.mock("./ContentTree.vue", () => import("@/test/contentTreeStub"));
import ImageDialog, { IMAGE_REGISTRY, escapeAlt, type PendingImage } from "./ImageDialog.vue";
import MarkdownBodyEditor from "./MarkdownBodyEditor.vue";
import { useContentStore } from "./store";

const mounted: VueWrapper[] = [];
let urls = 0;
const revoke = vi.fn();
beforeEach(() => {
  document.body.innerHTML = "";
  urls = 0;
  revoke.mockReset();
  URL.createObjectURL = vi.fn(() => `blob:http://localhost/preview-${++urls}`);
  URL.revokeObjectURL = revoke;
});
afterEach(() => { mounted.splice(0).forEach(w => w.unmount()); });

const png = (name = "Cat Photo.png") => new File([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], name, { type: "image/png" });
const svg = () => new File(["<svg/>"], "logo.svg", { type: "image/svg+xml" });
const $ = (selector: string) => document.body.querySelector(selector) as HTMLElement | null;
const dialogs = () => document.body.querySelectorAll('[data-testid="image-dialog"]').length;
const insertButton = () => $('[data-testid="image-insert"]') as HTMLButtonElement;

async function setAlt(text: string) {
  const input = $('[data-testid="image-alt"]') as HTMLInputElement;
  input.value = text; input.dispatchEvent(new Event("input")); await flushPromises();
}
async function pick(file: File) {
  const input = $('[data-testid="image-file"]') as HTMLInputElement;
  Object.defineProperty(input, "files", { value: [file], configurable: true });
  input.dispatchEvent(new Event("change")); await flushPromises();
}
async function insert(alt: string) { await setAlt(alt); insertButton().click(); await flushPromises(); }
function transfer(type: "paste" | "drop", file: File) {
  const event = new Event(type, { bubbles: true, cancelable: true });
  const data = { files: [file], types: ["Files"], getData: () => "" };
  Object.defineProperty(event, type === "paste" ? "clipboardData" : "dataTransfer", { value: data });
  return event;
}

describe("ImageDialog", () => {
  function open(file: File | null) {
    const w = mount(ImageDialog, { props: { open: true, file }, attachTo: document.body });
    mounted.push(w);
    return w;
  }

  it("shows one preview and cannot insert until alt text is non-empty", async () => {
    const file = png();
    const w = open(file); await flushPromises();
    expect(dialogs()).toBe(1);
    expect(($('[data-testid="image-preview"]') as HTMLImageElement).src).toBe("blob:http://localhost/preview-1");
    expect(insertButton().disabled).toBe(true);
    await setAlt("   ");
    expect(insertButton().disabled).toBe(true);
    await insert("  A cat ");
    const [image] = w.emitted("insert")![0] as [PendingImage];
    expect(image).toMatchObject({ alt: "A cat", filename: "Cat Photo.png", file, previewUrl: "blob:http://localhost/preview-1" });
    expect(image.clientId).toMatch(/^[A-Za-z0-9_-]{1,64}$/);
    expect(w.emitted("update:open")?.at(-1)).toEqual([false]);
    // The inserted preview belongs to the draft now; it is not revoked here.
    expect(revoke).not.toHaveBeenCalled();
  });

  it("chooses a file through the picker", async () => {
    open(null); await flushPromises();
    expect($('[data-testid="image-preview"]')).toBeNull();
    await pick(png());
    expect($('[data-testid="image-preview"]')).not.toBeNull();
    await setAlt("A cat");
    expect(insertButton().disabled).toBe(false);
  });

  it("refuses SVG and files over 10 MiB before upload and revokes previews it does not insert", async () => {
    const w = open(svg()); await flushPromises();
    expect($('[data-testid="image-error"]')?.textContent).toMatch(/PNG, JPEG, WebP, or GIF/);
    await setAlt("Logo");
    expect(insertButton().disabled).toBe(true);
    await pick(new File([new Uint8Array(10 * 1024 * 1024 + 1)], "huge.png", { type: "image/png" }));
    expect($('[data-testid="image-error"]')?.textContent).toMatch(/10 MiB/);
    expect(insertButton().disabled).toBe(true);
    await pick(png());
    ($('[data-testid="image-cancel"]') as HTMLButtonElement).click(); await flushPromises();
    expect(w.emitted("insert")).toBeUndefined();
    expect(revoke).toHaveBeenCalledWith("blob:http://localhost/preview-1");
  });

  it("escapes alt text for Markdown", () => {
    expect(escapeAlt("A [red] \\ cat\nline")).toBe("A \\[red\\] \\\\ cat line");
  });
});

describe("body editor image inputs", () => {
  async function editor(body: string, rich: boolean) {
    const add = vi.fn();
    const w: VueWrapper = mount(MarkdownBodyEditor, {
      props: { body, original: body, kind: "markdown", rich, onChange: (v: { body: string }) => w.setProps({ body: v.body }) },
      global: { provide: { [IMAGE_REGISTRY as symbol]: { add } } },
      attachTo: document.body,
    });
    mounted.push(w);
    await flushPromises();
    return { w, add, last: () => (w.emitted("change") ?? []).map(e => (e[0] as { body: string }).body).at(-1) };
  }
  const rawText = (w: VueWrapper) => EditorView.findFromDOM(w.get('[data-testid="body-raw"] .cm-editor').element as HTMLElement)!.state.doc.toString();

  it("paste in rich mode opens the dialog and inserts the preview with its alt text", async () => {
    const { w, add, last } = await editor("Hello\n", true);
    w.get('[data-testid="body-rich"] .ProseMirror').element.dispatchEvent(transfer("paste", png()));
    await flushPromises();
    expect(dialogs()).toBe(1);
    expect(last()).toBeUndefined();
    await insert("A [red] cat");
    expect(last()).toContain("![A \\[red\\] cat](blob:http://localhost/preview-1)");
    expect(add).toHaveBeenCalledWith(expect.objectContaining({ alt: "A [red] cat", previewUrl: "blob:http://localhost/preview-1" }));
  });

  it("drop in raw mode opens one dialog and inserts escaped Markdown", async () => {
    const { w, add } = await editor("Hello\n", false);
    const target = w.get('[data-testid="body-raw"] .cm-content').element;
    target.dispatchEvent(transfer("drop", png()));
    target.dispatchEvent(transfer("drop", png("other.png")));
    await flushPromises();
    expect(dialogs()).toBe(1);
    await insert("A [cat]");
    expect(rawText(w)).toContain("![A \\[cat\\]](blob:http://localhost/preview-1)");
    expect(add).toHaveBeenCalledTimes(1);
  });

  it("blank alt inserts nothing; cancelling leaves the body alone", async () => {
    const { w, last } = await editor("Hello\n", false);
    w.get('[data-testid="body-raw"] .cm-content').element.dispatchEvent(transfer("drop", png()));
    await flushPromises();
    await setAlt(" ");
    expect(insertButton().disabled).toBe(true);
    ($('[data-testid="image-cancel"]') as HTMLButtonElement).click(); await flushPromises();
    expect(last()).toBeUndefined();
    expect(rawText(w)).toBe("Hello\n");
  });
});

describe("pending images in the draft", () => {
  const collection = { name: "blog", kind: "markdown", editable: true, reason: null, fields: { type: "object", required: true, properties: {
    title: { type: "string", required: true },
    coverImage: { type: "image", required: false }, coverAlt: { type: "string", required: false },
  } } };
  const entry = (over = {}) => ({ collection: "blog", id: "welcome", path: "src/content/blog/en/welcome.md", kind: "markdown", source: "S", fields: { title: "Welcome" }, body: "Body\n", branch: "content/a", hash: "h1", ...over });
  let w: VueWrapper;
  async function setup(api: Record<string, unknown> = {}) {
    h.daemon = createFakeDaemon({
      listContentCollections: async () => [collection], listContentEntries: async () => [entry()],
      contentEntry: async () => entry(), updateContentEntry: vi.fn(async () => entry({ hash: "h2" })), ...api,
    });
    setActivePinia(createPinia());
    const router = await contentRouter();
    w = mount(RouterView, { global: { plugins: [router] }, attachTo: document.body }); mounted.push(w);
    await flushPromises();
    await w.get('[data-testid="content-collection"]').trigger("click"); await flushPromises();
    await w.get('[data-testid="content-entry"]').trigger("click"); await flushPromises();
    await vi.waitFor(() => w.get('[data-testid="body-editor"]'));
    await flushPromises();
  }
  const update = () => (h.daemon.api as unknown as { updateContentEntry: ReturnType<typeof vi.fn> }).updateContentEntry;
  async function addBodyImage(alt: string) {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    await (w.findComponent({ name: "MarkdownBodyEditor" }).vm as any).setMode("raw"); await flushPromises();
    w.get('[data-testid="body-raw"] .cm-content').element.dispatchEvent(transfer("drop", png()));
    await flushPromises();
    await insert(alt);
  }
  const save = async () => { await w.get('[data-testid="content-save"]').trigger("submit"); await flushPromises(); };

  it("holds local previews until Save, then uploads them with tokens in place of previews", async () => {
    await setup();
    await addBodyImage("A cat");
    const store = useContentStore();
    expect(update()).not.toHaveBeenCalled();
    expect(store.dirty).toBe(true);
    expect(store.pendingImages.map(i => i.previewUrl)).toEqual(["blob:http://localhost/preview-1"]);
    await save();
    const [body, images] = update().mock.calls[0]!;
    const clientId = store.pendingImages[0]?.clientId ?? images[0].clientId;
    expect(body.body).toContain(`![A cat](orso-image:${clientId})`);
    expect(body.body).not.toContain("blob:");
    expect(images).toEqual([expect.objectContaining({ clientId, alt: "A cat", filename: "Cat Photo.png", file: expect.any(File) })]);
    expect(store.pendingImages).toEqual([]);
    expect(store.dirty).toBe(false);
    expect(revoke).toHaveBeenCalledWith("blob:http://localhost/preview-1");
  });

  it("retains pending images and the draft after a failed save", async () => {
    const refused = new ApiError("BADREQ", "Cat Photo.png: The file is not a valid PNG image", 400, { reason: "unsupported_image" });
    await setup({ updateContentEntry: vi.fn(async () => { throw refused; }) });
    await addBodyImage("A cat");
    await save();
    const store = useContentStore();
    expect(store.saveError).toContain("not a valid PNG");
    expect(store.draftBody).toContain("![A cat](blob:http://localhost/preview-1)");
    expect(store.pendingImages).toHaveLength(1);
    expect(store.dirty).toBe(true);
    expect(revoke).not.toHaveBeenCalled();
    await save();
    expect(update()).toHaveBeenCalledTimes(2);
    expect(update().mock.calls[1]![1]).toHaveLength(1);
  });

  it("an image field fills its value and alt companion from the dialog", async () => {
    await setup();
    const upload = w.get('[data-testid="field-coverImage-uploader"] input[type="file"]');
    Object.defineProperty(upload.element, "files", { value: [png("cover.png")], configurable: true });
    await upload.trigger("change"); await flushPromises();
    expect(($('[data-testid="image-preview"]') as HTMLImageElement).src).toBe("blob:http://localhost/preview-1");
    await insert("Team photo");
    const store = useContentStore();
    expect(store.draftFields.coverImage).toBe("blob:http://localhost/preview-1");
    expect(store.draftFields.coverAlt).toBe("Team photo");
    expect(w.get('[data-testid="field-coverImage-uploader"] img').attributes("src")).toBe("blob:http://localhost/preview-1");
    await save();
    const [body, images] = update().mock.calls[0]!;
    expect(body.fields).toEqual({ coverImage: `orso-image:${images[0].clientId}`, coverAlt: "Team photo" });
  });

  it("drops a pending image the writer removed before saving", async () => {
    await setup();
    await addBodyImage("A cat");
    const store = useContentStore();
    store.setBody("Body\n");
    expect(store.dirty).toBe(false);
  });
});
