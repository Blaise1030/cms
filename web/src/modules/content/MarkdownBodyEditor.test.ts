// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { EditorView } from "@codemirror/view";
import MarkdownBodyEditor from "./MarkdownBodyEditor.vue";
import { canRoundTrip } from "./markdown";

type Change = { body: string; dirty: boolean };
const mounted: VueWrapper[] = [];
afterEach(() => { mounted.splice(0).forEach(w => w.unmount()); vi.unstubAllGlobals(); });

async function render(body: string, opts: { kind?: "markdown" | "mdx"; rich?: boolean } = {}) {
  const w: VueWrapper = mount(MarkdownBodyEditor, {
    // Like the store, the parent feeds each emitted body back in as the draft.
    props: { body, original: body, kind: opts.kind ?? "markdown", rich: opts.rich ?? true, onChange: (v: Change) => w.setProps({ body: v.body }) },
    attachTo: document.body,
  });
  mounted.push(w);
  await flushPromises();
  return w;
}
const changes = (w: VueWrapper) => (w.emitted("change") ?? []).map(e => e[0] as Change);
const last = (w: VueWrapper) => changes(w).at(-1);
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const editorOf = (w: VueWrapper) => (w.vm as any).richEditor();
const rawView = (w: VueWrapper) => EditorView.findFromDOM(w.get('[data-testid="body-raw"] .cm-editor').element as HTMLElement)!;
const rawText = (w: VueWrapper) => rawView(w).state.doc.toString();
function typeRaw(w: VueWrapper, text: string) {
  const view = rawView(w);
  view.dispatch({ changes: { from: view.state.doc.length, insert: text }, userEvent: "input.type" });
}
async function mode(w: VueWrapper, which: "rich" | "raw") {
  await w.get(`[data-testid="body-mode-${which}"]`).trigger("click");
  await flushPromises();
}
async function selectAll(w: VueWrapper) { editorOf(w).commands.selectAll(); await flushPromises(); }
/** Choose a PNG in the open image dialog and type its alt text. */
async function pickImage(alt: string) {
  URL.createObjectURL = vi.fn(() => "blob:preview");
  URL.revokeObjectURL = vi.fn();
  const input = document.body.querySelector('[data-testid="image-file"]') as HTMLInputElement;
  Object.defineProperty(input, "files", { value: [new File([new Uint8Array([0x89])], "cat.png", { type: "image/png" })], configurable: true });
  input.dispatchEvent(new Event("change"));
  const text = document.body.querySelector('[data-testid="image-alt"]') as HTMLInputElement;
  text.value = alt; text.dispatchEvent(new Event("input"));
  await flushPromises();
}

describe("canRoundTrip", () => {
  it.each([
    "# Title\n\nSome **bold**, *em* and `code` with a [link](https://x.test).\n",
    "- one\n- two\n\n1. first\n2. second\n",
    "> quoted\n\n```ts\nconst a = { b: 1 };\n```\n",
    "![A cat](./cat.png)\n",
    "",
  ])("accepts supported Markdown %#", source => expect(canRoundTrip(source)).toBe(true));

  it.each([
    ["block HTML", "Hi\n\n<div class=\"box\">Box</div>\n"],
    ["inline HTML", "Hi <span>there</span>\n"],
    ["HTML comment", "<!-- note -->\nText\n"],
    ["MDX component", "<Callout type=\"tip\">Hi</Callout>\n"],
    ["MDX import", "import Chart from '../Chart.astro';\n\nText\n"],
    ["MDX export", "export const meta = {};\n\nText\n"],
    ["MDX expression", "Total: {props.count}\n"],
    ["table", "| a | b |\n| - | - |\n| 1 | 2 |\n"],
    ["reference link", "See [docs][d].\n\n[d]: https://x.test\n"],
    ["footnote", "Claim[^1].\n\n[^1]: Source.\n"],
    ["entity", "Fish &amp; chips\n"],
    ["task list", "- [ ] todo\n"],
  ])("rejects %s", (_name, source) => expect(canRoundTrip(source)).toBe(false));
});

describe("MarkdownBodyEditor", () => {
  it("opens supported Markdown in rich mode without emitting a change", async () => {
    const w = await render("# Title\n\nBody text\n");
    expect(w.find('[data-testid="body-rich"]').exists()).toBe(true);
    expect(w.find('[data-testid="body-toolbar"]').exists()).toBe(true);
    expect(changes(w)).toEqual([]);
  });

  it.each([
    ["heading", "## Hello"],
    ["bullet-list", "- Hello"],
    ["ordered-list", "1. Hello"],
    ["quote", "> Hello"],
    ["code-block", "```\nHello\n```"],
    ["bold", "**Hello**"],
    ["italic", "*Hello*"],
    ["code", "`Hello`"],
  ])("toolbar %s rewrites the selection", async (tool, expected) => {
    const w = await render("Hello");
    await selectAll(w);
    await w.get(`[data-testid="body-tool-${tool}"]`).trigger("click"); await flushPromises();
    expect(last(w)).toEqual({ body: expected, dirty: true });
  });

  it("toolbar link asks for a URL", async () => {
    vi.stubGlobal("prompt", vi.fn(() => "https://x.test"));
    const w = await render("Hello");
    await selectAll(w);
    await w.get('[data-testid="body-tool-link"]').trigger("click"); await flushPromises();
    expect(last(w)?.body).toBe("[Hello](https://x.test)");
  });

  it("toolbar image opens the image dialog and needs non-empty alt text", async () => {
    const w = await render("Hello");
    await w.get('[data-testid="body-tool-image"]').trigger("click"); await flushPromises();
    await pickImage(" ");
    expect((document.body.querySelector('[data-testid="image-insert"]') as HTMLButtonElement).disabled).toBe(true);
    await pickImage("A cat");
    (document.body.querySelector('[data-testid="image-insert"]') as HTMLButtonElement).click(); await flushPromises();
    expect(last(w)?.body).toContain("![A cat](blob:preview)");
  });

  it.each([
    ["Heading", "## Title"],
    ["Bullet list", "- Title"],
    ["Numbered list", "1. Title"],
    ["Quote", "> Title"],
    ["Code block", "```\nTitle\n```"],
  ])("slash command %s formats the block", async (label, expected) => {
    const w = await render("");
    editorOf(w).chain().focus().insertContent("/").run(); await flushPromises();
    const item = w.findAll('[data-testid="slash-item"]').find(i => i.text().includes(label));
    expect(item, `slash item ${label}`).toBeTruthy();
    await item!.trigger("mousedown"); await flushPromises();
    editorOf(w).commands.insertContent("Title"); await flushPromises();
    expect(last(w)?.body).toBe(expected);
  });

  it("slash commands filter by query and cover links and images", async () => {
    const w = await render("");
    editorOf(w).chain().focus().insertContent("/").run(); await flushPromises();
    const labels = w.findAll('[data-testid="slash-item"]').map(i => i.text());
    for (const label of ["Heading", "Bullet list", "Numbered list", "Link", "Quote", "Code block", "Image"])
      expect(labels.some(l => l.includes(label))).toBe(true);
    editorOf(w).commands.insertContent("ima"); await flushPromises();
    expect(w.findAll('[data-testid="slash-item"]').map(i => i.text())).toEqual([expect.stringContaining("Image")]);
    await w.get('[data-testid="slash-item"]').trigger("mousedown"); await flushPromises();
    await pickImage("A cat");
    (document.body.querySelector('[data-testid="image-insert"]') as HTMLButtonElement).click(); await flushPromises();
    expect(last(w)?.body).toContain("![A cat](blob:preview)");
    expect(last(w)?.body).not.toContain("/ima");
  });

  it("keeps the exact original bytes through unchanged open and toggles", async () => {
    // Supported but non-canonical: setext heading, `*` bullets, `__strong__`, trailing spaces.
    const source = "Title\n=====\n\n* one\n* two\n\n__strong__ text   \n\n\n";
    expect(canRoundTrip(source)).toBe(true);
    const w = await render(source);
    expect(w.find('[data-testid="body-rich"]').exists()).toBe(true);
    await mode(w, "raw"); expect(rawText(w)).toBe(source);
    await mode(w, "rich"); await mode(w, "raw"); expect(rawText(w)).toBe(source);
    await mode(w, "rich");
    expect(changes(w)).toEqual([]);
  });

  it("rich to raw to rich keeps supported content and raw edits", async () => {
    const source = "# Title\n\nSome **bold** text.\n\n- a\n- b\n";
    const w = await render(source);
    await selectAll(w);
    editorOf(w).chain().setTextSelection(editorOf(w).state.doc.content.size - 1).insertContent("c").run(); await flushPromises();
    const richBody = last(w)!.body;
    expect(richBody).toContain("# Title"); expect(richBody).toContain("**bold**"); expect(richBody).toContain("- bc");
    await mode(w, "raw");
    expect(rawText(w)).toBe(richBody);
    typeRaw(w, "\n\n> Added in raw\n"); await flushPromises();
    expect(last(w)).toEqual({ body: `${richBody}\n\n> Added in raw\n`, dirty: true });
    await mode(w, "rich");
    const html = editorOf(w).getHTML();
    expect(html).toContain("<h1>Title</h1>"); expect(html).toContain("<strong>bold</strong>"); expect(html).toContain("<blockquote>");
    // Toggling alone does not reserialize the raw text.
    expect(last(w)?.body).toBe(`${richBody}\n\n> Added in raw\n`);
    await mode(w, "raw");
    expect(rawText(w)).toBe(`${richBody}\n\n> Added in raw\n`);
  });

  it("reports clean again when an edit returns to the original", async () => {
    const w = await render("Hello\n", { rich: false });
    typeRaw(w, "!"); await flushPromises();
    expect(last(w)).toEqual({ body: "Hello\n!", dirty: true });
    const view = rawView(w);
    view.dispatch({ changes: { from: view.state.doc.length - 1, to: view.state.doc.length }, userEvent: "delete" }); await flushPromises();
    expect(last(w)).toEqual({ body: "Hello\n", dirty: false });
  });

  it("opens unsupported HTML/MDX raw and refuses rich mode without losing it", async () => {
    const source = "Intro\n\n<div class=\"box\">Keep me</div>\n";
    const w = await render(source);
    expect(w.find('[data-testid="body-rich"]').exists()).toBe(false);
    expect(rawText(w)).toBe(source);
    expect(w.get('[data-testid="body-unsupported"]').text()).toMatch(/raw/i);
    await mode(w, "rich");
    expect(w.find('[data-testid="body-rich"]').exists()).toBe(false);
    expect(rawText(w)).toBe(source);
    expect(changes(w)).toEqual([]);
  });

  it("stays raw when a raw edit introduces unsupported Markdown", async () => {
    const w = await render("Intro\n");
    await mode(w, "raw");
    typeRaw(w, "\n<Callout>Tip</Callout>\n"); await flushPromises();
    await mode(w, "rich");
    expect(w.find('[data-testid="body-rich"]').exists()).toBe(false);
    expect(w.find('[data-testid="body-unsupported"]').exists()).toBe(true);
    expect(rawText(w)).toBe("Intro\n\n<Callout>Tip</Callout>\n");
    expect(last(w)?.body).toBe("Intro\n\n<Callout>Tip</Callout>\n");
  });

  it("keeps section MDX raw-only", async () => {
    const source = "Plain words only\n";
    const w = await render(source, { kind: "mdx", rich: true });
    expect(w.find('[data-testid="body-mode-rich"]').exists()).toBe(false);
    expect(w.find('[data-testid="body-toolbar"]').exists()).toBe(false);
    expect(rawText(w)).toBe(source);
  });

  it("follows an external reset of the draft body", async () => {
    const w = await render("Hello\n", { rich: false });
    typeRaw(w, "edited"); await flushPromises();
    await w.setProps({ body: "Hello\n", original: "Hello\n" }); await flushPromises();
    expect(rawText(w)).toBe("Hello\n");
  });
});
