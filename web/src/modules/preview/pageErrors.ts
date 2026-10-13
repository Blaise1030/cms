/** An error the framed page hit, described for the agent. */
export interface PageError {
  /** `build`: the dev server's error overlay (compile/SSR error). `runtime`: an uncaught error. */
  kind: "build" | "runtime";
  message: string;
  /** `file:line:col`, when known. */
  file?: string;
  stack?: string;
  /** The overlay's suggestion (Astro). */
  hint?: string;
}

/** Vite's error overlay; Astro replaces its contents but keeps the tag. */
const OVERLAY = "vite-error-overlay";
const MAX_MESSAGE = 1000;
const MAX_STACK_LINES = 12;

function clip(
  text: string | undefined | null,
  max = MAX_MESSAGE,
): string | undefined {
  const t = text?.trim();
  if (!t) return undefined;
  return t.length > max ? `${t.slice(0, max)}…` : t;
}

function clipStack(stack: string | undefined | null): string | undefined {
  const lines = stack?.trim().split("\n");
  if (!lines?.length || !lines[0]) return undefined;
  return lines.length > MAX_STACK_LINES
    ? [...lines.slice(0, MAX_STACK_LINES), "    …"].join("\n")
    : lines.join("\n");
}

/** The first matching element's text in the overlay's shadow root. */
function pick(root: ParentNode, selectors: string[]): string | undefined {
  for (const s of selectors) {
    const text = clip(root.querySelector(s)?.textContent);
    if (text) return text;
  }
  return undefined;
}

/** Read an error overlay: Astro's ids first, then Vite's classes. */
export function readOverlay(overlay: Element): PageError {
  const root: ParentNode = overlay.shadowRoot ?? overlay;
  const title = pick(root, ["#title"]);
  const body = pick(root, ["#message-content", ".message-body"]);
  const message =
    [title !== "An error occurred." ? title : undefined, body]
      .filter(Boolean)
      .join(": ") ||
    clip(root.textContent) ||
    "The page failed to build.";
  // Astro puts the absolute location in the code header's `title`.
  const file =
    clip(root.querySelector("#code header h2")?.getAttribute("title")) ??
    pick(root, ["#code header h2", ".file"]);
  return {
    kind: "build",
    message,
    file,
    stack: clipStack(pick(root, ["#stack-content", ".stack"])),
    hint: pick(root, ["#hint-content"]),
  };
}

function fromThrown(reason: unknown, fallback: string): PageError {
  if (
    reason instanceof Error ||
    (reason && typeof reason === "object" && "message" in reason)
  ) {
    const e = reason as Error;
    return {
      kind: "runtime",
      message: clip(`${e.name ?? "Error"}: ${e.message}`)!,
      stack: clipStack(e.stack),
    };
  }
  return { kind: "runtime", message: clip(String(reason ?? "")) ?? fallback };
}

export interface PageErrorHandlers {
  onError(error: PageError): void;
  /** The error overlay went away (Vite removes it after a successful update). */
  onOverlayCleared(): void;
}

/** Watch a same-origin frame for an error overlay and uncaught errors. Returns a detach. */
export function watchPageErrors(
  win: Window,
  handlers: PageErrorHandlers,
): () => void {
  const doc = win.document;
  let overlay: Element | null = null;

  const scan = () => {
    const found = doc.querySelector(OVERLAY);
    if (found === overlay) return;
    overlay = found;
    if (found) handlers.onError(readOverlay(found));
    else handlers.onOverlayCleared();
  };

  const onError = (e: ErrorEvent) => {
    const error = e.error
      ? fromThrown(e.error, e.message)
      : fromThrown(e.message, "Script error");
    if (e.filename)
      error.file = [e.filename, e.lineno, e.colno].filter(Boolean).join(":");
    handlers.onError(error);
  };
  const onRejection = (e: PromiseRejectionEvent) =>
    handlers.onError(fromThrown(e.reason, "Unhandled promise rejection"));

  win.addEventListener("error", onError);
  win.addEventListener("unhandledrejection", onRejection);
  const observer = new MutationObserver(scan);
  observer.observe(doc.documentElement, { childList: true, subtree: true });
  if (doc.querySelector(OVERLAY)) scan();

  return () => {
    observer.disconnect();
    win.removeEventListener("error", onError);
    win.removeEventListener("unhandledrejection", onRejection);
  };
}

function fence(text: string): string {
  return "```\n" + text + "\n```";
}

/** The request pasted into the agent's input for a page error. */
export function pageErrorPrompt(error: PageError, path: string): string {
  const what =
    error.kind === "build" ? "fails to build" : "throws an uncaught error";
  const details = [
    error.message,
    error.file && `at ${error.file}`,
    error.stack && error.stack !== error.message ? error.stack : undefined,
  ].filter(Boolean);
  const lines = [
    `The preview page ${path} ${what}:`,
    fence(details.join("\n")),
  ];
  if (error.hint) lines.push(`Hint: ${error.hint}`);
  lines.push("Please find the cause and fix it.");
  return lines.join("\n");
}

/** The request pasted into the agent's input when the dev server crashed. */
export function crashPrompt(message: string | undefined): string {
  const lines = ["The dev server crashed."];
  if (message?.trim()) lines.push(fence(clip(message, 4000)!));
  lines.push(
    "Please find the cause and fix it, so the dev server starts again.",
  );
  return lines.join("\n");
}
