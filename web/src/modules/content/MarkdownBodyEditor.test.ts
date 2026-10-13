// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { EditorView } from "@codemirror/view";
import MarkdownBodyEditor from "./MarkdownBodyEditor.vue";
import { Editor } from "@tiptap/core";
import { canRoundTrip, markdownExtensions } from "./markdown";

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
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  await (w.vm as any).setMode(which);
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
    "| a | **b** |\n| :- | -: |\n| 1 | [two](https://x.test) |\n",
    "Is it allowed?\n\nNo. Meta restricts it.\n\nA. Yes, sometimes.\n",
    "## Voice & Tone\n\nFish & chips, R&D.\n",
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
    ["HTML in a table cell", "| a | b |\n| - | - |\n| <b>1</b> | 2 |\n"],
    ["reference link", "See [docs][d].\n\n[d]: https://x.test\n"],
    ["footnote", "Claim[^1].\n\n[^1]: Source.\n"],
    ["entity", "Fish &amp; chips\n"],
    ["task list", "- [ ] todo\n"],
  ])("rejects %s", (_name, source) => expect(canRoundTrip(source)).toBe(false));

  it("writes a bare ampersand as-is, and escapes one that would read as an entity", () => {
    const md = new Editor({ extensions: markdownExtensions() }).markdown!;
    const doc = (text: string) => ({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] });
    expect(md.serialize(doc("R&D & more"))).toBe("R&D & more");
    expect(md.serialize(doc("Type &amp; or &#38;"))).toBe("Type &amp;amp; or &amp;#38;");
  });
});

describe("MarkdownBodyEditor", () => {
  it("opens supported Markdown in rich mode without emitting a change", async () => {
    const w = await render("# Title\n\nBody text\n");
    expect(w.find('[data-testid="body-rich"]').exists()).toBe(true);
    expect(w.find('[data-testid="body-bubble"]').exists()).toBe(false);
    expect(changes(w)).toEqual([]);
  });

  it("shows the selection menu only while text is selected", async () => {
    const w = await render("Hello");
    await selectAll(w);
    expect(w.find('[data-testid="body-bubble"]').exists()).toBe(true);
    expect((w.get('[data-testid="body-block-type"]').element as HTMLSelectElement).value).toBe("text");
    editorOf(w).commands.setTextSelection(1); await flushPromises();
    expect(w.find('[data-testid="body-bubble"]').exists()).toBe(false);
  });

  it.each([
    ["h1", "# Hello"],
    ["h2", "## Hello"],
    ["bullet-list", "- Hello"],
    ["ordered-list", "1. Hello"],
    ["quote", "> Hello"],
    ["code-block", "```\nHello\n```"],
  ])("block type %s rewrites the selection", async (block, expected) => {
    const w = await render("Hello");
    await selectAll(w);
    await w.get('[data-testid="body-block-type"]').setValue(block); await flushPromises();
    expect(last(w)).toEqual({ body: expected, dirty: true });
  });

  it("block type Text turns a heading back into a paragraph", async () => {
    const w = await render("## Hello");
    await selectAll(w);
    expect((w.get('[data-testid="body-block-type"]').element as HTMLSelectElement).value).toBe("h2");
    await w.get('[data-testid="body-block-type"]').setValue("text"); await flushPromises();
    expect(last(w)).toEqual({ body: "Hello", dirty: true });
  });

  it.each([
    ["bold", "**Hello**"],
    ["italic", "*Hello*"],
    ["code", "`Hello`"],
  ])("selection menu %s rewrites the selection", async (tool, expected) => {
    const w = await render("Hello");
    await selectAll(w);
    await w.get(`[data-testid="body-tool-${tool}"]`).trigger("click"); await flushPromises();
    expect(last(w)).toEqual({ body: expected, dirty: true });
  });

  it("selection menu clear removes formatting", async () => {
    const w = await render("**Hello**");
    await selectAll(w);
    await w.get('[data-testid="body-tool-clear"]').trigger("click"); await flushPromises();
    expect(last(w)).toEqual({ body: "Hello", dirty: true });
  });

  /** Type into a field of the open link dialog. */
  async function fillLink(field: "url" | "text", value: string) {
    const input = document.body.querySelector(`[data-testid="link-${field}"]`) as HTMLInputElement;
    input.value = value; input.dispatchEvent(new Event("input"));
    await flushPromises();
  }
  const linkButton = (name: string) => document.body.querySelector(`[data-testid="link-${name}"]`) as HTMLButtonElement | null;

  it("shows the link card when the cursor is on a link, and hides it off the link", async () => {
    const w = await render("See [docs](https://x.test) here");
    expect(w.find('[data-testid="link-popover"]').exists()).toBe(false);
    editorOf(w).commands.setTextSelection(7); await flushPromises();
    expect(w.get('[data-testid="link-open"]').attributes("href")).toBe("https://x.test");
    expect(w.find('[data-testid="link-url"]').exists()).toBe(false);
    editorOf(w).commands.setTextSelection(2); await flushPromises();
    expect(w.find('[data-testid="link-popover"]').exists()).toBe(false);
    expect(changes(w)).toEqual([]);
  });

  it("link card Edit switches to the URL field and Escape cancels", async () => {
    const w = await render("See [docs](https://x.test) here");
    editorOf(w).commands.setTextSelection(7); await flushPromises();
    await w.get('[data-testid="link-edit"]').trigger("click"); await flushPromises();
    expect((w.get('[data-testid="link-url"]').element as HTMLInputElement).value).toBe("https://x.test");
    await w.get('[data-testid="link-url"]').trigger("keydown", { key: "Escape" }); await flushPromises();
    expect(w.find('[data-testid="link-popover"]').exists()).toBe(false);
    expect(changes(w)).toEqual([]);
  });

  it("selection menu link asks for a URL in a popover", async () => {
    const w = await render("Hello");
    await selectAll(w);
    await w.get('[data-testid="body-tool-link"]').trigger("click"); await flushPromises();
    expect(linkButton("apply")!.disabled).toBe(true);
    expect(document.body.querySelector('[data-testid="link-text"]')).toBeNull();
    await fillLink("url", "https://x.test");
    linkButton("apply")!.click(); await flushPromises();
    expect(last(w)?.body).toBe("[Hello](https://x.test)");
  });

  it("link popover edits and removes an existing link", async () => {
    const w = await render("[Hello](https://old.test)");
    await selectAll(w);
    await w.get('[data-testid="body-tool-link"]').trigger("click"); await flushPromises();
    expect((document.body.querySelector('[data-testid="link-url"]') as HTMLInputElement).value).toBe("https://old.test");
    await fillLink("url", "https://new.test");
    linkButton("apply")!.click(); await flushPromises();
    expect(last(w)?.body).toBe("[Hello](https://new.test)");
    expect(w.get('[data-testid="link-open"]').attributes("href")).toBe("https://new.test");

    linkButton("remove")!.click(); await flushPromises();
    expect(last(w)?.body).toBe("Hello");
  });

  it("slash link with no selection asks for the link text too", async () => {
    const w = await render("Hi ");
    editorOf(w).chain().focus().setTextSelection(4).insertContent("/").run(); await flushPromises();
    (document.body.querySelector('[data-testid="slash-item"]:nth-child(4)') as HTMLElement)
      .dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    await flushPromises();
    await fillLink("url", "https://x.test");
    await fillLink("text", "docs");
    linkButton("apply")!.click(); await flushPromises();
    expect(last(w)?.body).toBe("Hi [docs](https://x.test)");
  });

  /** Put the cursor at the end of the text node `text`. */
  function cursorAfter(w: VueWrapper, text: string) {
    let at = 0;
    editorOf(w).state.doc.descendants((node: { isText: boolean; text?: string; nodeSize: number }, pos: number) => { if (node.isText && node.text === text) at = pos + node.nodeSize; });
    editorOf(w).chain().focus().setTextSelection(at).run();
  }
  async function altArrow(w: VueWrapper, key: "ArrowUp" | "ArrowDown") {
    editorOf(w).view.dom.dispatchEvent(new KeyboardEvent("keydown", { key, altKey: true, bubbles: true, cancelable: true }));
    await flushPromises();
  }

  it("Alt+Up and Alt+Down move the current block; the last block stays put", async () => {
    const w = await render("One\n\nTwo\n\nThree");
    cursorAfter(w, "Three");
    await altArrow(w, "ArrowDown");
    expect(last(w)).toBeUndefined();
    await altArrow(w, "ArrowUp");
    expect(last(w)?.body).toBe("One\n\nThree\n\nTwo");
    await altArrow(w, "ArrowUp");
    expect(last(w)?.body).toBe("Three\n\nOne\n\nTwo");
    await altArrow(w, "ArrowDown");
    expect(last(w)?.body).toBe("One\n\nThree\n\nTwo");
  });

  it("Alt+Up moves a list item within its list", async () => {
    const w = await render("- a\n- b\n- c");
    cursorAfter(w, "c");
    await altArrow(w, "ArrowUp");
    expect(last(w)?.body).toBe("- a\n- c\n- b");
    await altArrow(w, "ArrowUp");
    expect(last(w)?.body).toBe("- c\n- a\n- b");
  });

  it("slash image needs non-empty alt text", async () => {
    const w = await render("");
    editorOf(w).chain().focus().insertContent("/image").run(); await flushPromises();
    await w.get('[data-testid="slash-item"]').trigger("mousedown"); await flushPromises();
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
    expect(w.find('[data-testid="body-rich"]').exists()).toBe(false);
    await mode(w, "rich");
    expect(w.find('[data-testid="body-rich"]').exists()).toBe(false);
    expect(rawText(w)).toBe(source);
  });

  it("follows an external reset of the draft body", async () => {
    const w = await render("Hello\n", { rich: false });
    typeRaw(w, "edited"); await flushPromises();
    await w.setProps({ body: "Hello\n", original: "Hello\n" }); await flushPromises();
    expect(rawText(w)).toBe("Hello\n");
  });
});

describe("saved image links", () => {
  it("resolves entry-relative image paths and leaves other links alone", async () => {
    const { entryAssetPath } = await import("./markdown");
    const entry = "src/content/blog/en/spring.md";
    expect(entryAssetPath(entry, "../../../assets/blog/spring/team.png")).toBe("src/assets/blog/spring/team.png");
    expect(entryAssetPath(entry, "./a%20b.png")).toBe("src/content/blog/en/a b.png");
    expect(entryAssetPath(entry, "../../../../../../etc/passwd")).toBeNull();
    for (const src of ["https://x.test/a.png", "blob:abc", "data:image/png;base64,AA", "/abs.png", ""]) expect(entryAssetPath(entry, src)).toBeNull();
  });

  it("shows a resolved src in rich mode while the Markdown keeps the relative link", async () => {
    const body = "Intro\n\n![Team](../../../assets/blog/spring/team.png)\n";
    const w = mount(MarkdownBodyEditor, {
      props: { body, original: body, kind: "markdown", rich: true, resolveImage: (src: string) => `http://daemon/raw?path=${encodeURIComponent(src)}` },
      attachTo: document.body,
    });
    mounted.push(w);
    await flushPromises();
    const img = w.get('[data-testid="body-rich"] img').element as HTMLImageElement;
    expect(img.getAttribute("src")).toBe(`http://daemon/raw?path=${encodeURIComponent("../../../assets/blog/spring/team.png")}`);
    expect(img.getAttribute("alt")).toBe("Team");
    expect(editorOf(w).getMarkdown()).toContain("![Team](../../../assets/blog/spring/team.png)");
    editorOf(w).commands.insertContentAt(0, "New ");
    await flushPromises();
    expect(last(w)?.body).toContain("![Team](../../../assets/blog/spring/team.png)");
  });
});

describe("saved image links survive copy and paste", () => {
  it("re-parses the written link, not the display URL, from the editor's HTML", async () => {
    const body = "![Team](../../../assets/blog/spring/team.png)\n";
    const w = mount(MarkdownBodyEditor, {
      props: { body, original: body, kind: "markdown", rich: true, resolveImage: (src: string) => `http://daemon/api/file?path=${encodeURIComponent(src)}&raw=true` },
      attachTo: document.body,
    });
    mounted.push(w);
    await flushPromises();
    const editor = editorOf(w);
    expect(editor.getHTML()).toContain('data-src="../../../assets/blog/spring/team.png"');
    // What a copy then paste does: the copied HTML is parsed back into the document.
    editor.commands.insertContentAt(editor.state.doc.content.size, editor.getHTML());
    await flushPromises();
    const markdown = editor.getMarkdown();
    expect(markdown.match(/!\[Team\]\(\.\.\/\.\.\/\.\.\/assets\/blog\/spring\/team\.png\)/g)).toHaveLength(2);
    expect(markdown).not.toContain("api/file");
  });

  it("keeps a pasted outside image's src", async () => {
    const w = await render("Text\n");
    const editor = editorOf(w);
    editor.commands.insertContentAt(editor.state.doc.content.size, '<img src="https://example.com/a.png" alt="A">');
    await flushPromises();
    expect(editor.getMarkdown()).toContain("![A](https://example.com/a.png)");
  });

  it("leaves malformed escapes unresolved instead of breaking the editor", async () => {
    const { entryAssetPath } = await import("./markdown");
    expect(entryAssetPath("src/content/blog/en/a.md", "100%.png")).toBeNull();
    const body = "![x](100%.png)\n";
    const resolveImage = vi.fn((src: string) => `http://daemon/${src}`);
    const w = mount(MarkdownBodyEditor, { props: { body, original: body, kind: "markdown", rich: true, resolveImage: (src: string) => {
      const path = entryAssetPath("src/content/blog/en/a.md", src); return path ? resolveImage(path) : src;
    } }, attachTo: document.body });
    mounted.push(w);
    await flushPromises();
    expect(w.find('[data-testid="body-rich"] img').attributes("src")).toBe("100%.png");
    expect(resolveImage).not.toHaveBeenCalled();
  });
});
