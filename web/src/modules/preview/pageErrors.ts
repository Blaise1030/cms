/**
 * An error the framed page hit, described for the agent. The preview is cross-origin, so the
 * page's bridge (`packages/astro/src/bridge.js`) watches for the dev server's error overlay and
 * uncaught errors and reports them in this shape.
 */
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

function clip(text: string | undefined | null, max: number): string | undefined {
  const t = text?.trim();
  if (!t) return undefined;
  return t.length > max ? `${t.slice(0, max)}…` : t;
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
