/**
 * Self-capture for PR preview builds (`ORSO_PREVIEW=1`), injected with the preview bridge so
 * orso can screenshot an area of the deployed preview without asking to share the screen.
 *
 * The visible page is copied with its computed styles inlined and same-origin images inlined
 * as data URLs, drawn through an SVG `foreignObject` onto a canvas, and the requested area is
 * cut out. Known gaps: web fonts fall back to system fonts; CSS background images, pseudo
 * elements, video, canvas, iframes and cross-origin images don't show. The parent falls back
 * to another capture when this throws or comes out blank.
 */
async function __orsoCaptureRect(rect, maxSide = 1600) {
  const SVG_NS = "http://www.w3.org/2000/svg";
  const XHTML_NS = "http://www.w3.org/1999/xhtml";
  const width = innerWidth;
  const height = innerHeight;

  /** A copy of `el` with every element's computed style written inline. */
  function copy(el) {
    const clone = el.cloneNode(false);
    if (el.nodeType !== 1) return clone;
    const tag = el.tagName.toLowerCase();
    if (tag === "script" || tag === "noscript" || tag === "template") return null;
    const style = getComputedStyle(el);
    let css = "";
    for (let i = 0; i < style.length; i++) {
      const name = style[i];
      css += `${name}:${style.getPropertyValue(name)};`;
    }
    clone.setAttribute("style", css);
    clone.removeAttribute("class");
    for (const child of el.childNodes) {
      const c = copy(child);
      if (c) clone.append(c);
    }
    if (tag === "input" || tag === "textarea") clone.setAttribute("value", el.value);
    return clone;
  }

  /** `src` of every same-origin image as a data URL (others are left out of the picture). */
  async function inlineImages(root) {
    await Promise.all(
      [...root.querySelectorAll("img")].map(async (img) => {
        try {
          const src = img.currentSrc || img.src;
          if (!src || new URL(src, location.href).origin !== location.origin) throw new Error("cross-origin");
          const blob = await (await fetch(src)).blob();
          const url = await new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(reader.result);
            reader.onerror = () => reject(reader.error);
            reader.readAsDataURL(blob);
          });
          img.setAttribute("src", url);
          img.removeAttribute("srcset");
        } catch {
          img.removeAttribute("src");
          img.removeAttribute("srcset");
        }
      }),
    );
  }

  const body = copy(document.body);
  // The body becomes a block in an XHTML document, scrolled where the reader is.
  const page = document.createElementNS(XHTML_NS, "div");
  const htmlStyle = getComputedStyle(document.documentElement);
  page.setAttribute(
    "style",
    `width:${width}px;height:${height}px;overflow:hidden;background:${htmlStyle.backgroundColor || "#fff"};`,
  );
  const inner = document.createElementNS(XHTML_NS, "div");
  inner.setAttribute("style", `transform:translate(${-scrollX}px,${-scrollY}px);`);
  inner.append(body);
  page.append(inner);
  await inlineImages(page);

  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("width", String(width));
  svg.setAttribute("height", String(height));
  const fo = document.createElementNS(SVG_NS, "foreignObject");
  fo.setAttribute("width", "100%");
  fo.setAttribute("height", "100%");
  fo.append(page);
  svg.append(fo);
  const markup = new XMLSerializer().serializeToString(svg);
  const image = new Image();
  image.decoding = "sync";
  await new Promise((resolve, reject) => {
    image.onload = resolve;
    image.onerror = () => reject(new Error("The page couldn't be drawn."));
    image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(markup)}`;
  });

  const x = Math.max(0, Math.min(width, rect.x));
  const y = Math.max(0, Math.min(height, rect.y));
  const w = Math.max(1, Math.min(width - x, rect.width));
  const h = Math.max(1, Math.min(height - y, rect.height));
  const scale = Math.min(Math.max(devicePixelRatio || 1, 1), maxSide / Math.max(w, h));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(w * scale));
  canvas.height = Math.max(1, Math.round(h * scale));
  const ctx = canvas.getContext("2d");
  ctx.drawImage(image, x, y, w, h, 0, 0, canvas.width, canvas.height);
  if (isBlank(ctx, canvas.width, canvas.height)) throw new Error("The capture came out blank.");
  return canvas.toDataURL("image/png");
}

/** One colour everywhere: nothing was drawn worth sending. */
function isBlank(ctx, width, height) {
  const { data } = ctx.getImageData(0, 0, width, height);
  for (let i = 4; i < data.length; i += 4) {
    if (data[i] !== data[0] || data[i + 1] !== data[1] || data[i + 2] !== data[2] || data[i + 3] !== data[3]) {
      return false;
    }
  }
  return true;
}
