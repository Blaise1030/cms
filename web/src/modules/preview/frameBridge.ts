import type { PickedElement } from "./elementPicker";
import type { PageError } from "./pageErrors";

/** Tag on every `postMessage` between the app and the injected preview bridge. */
export const BRIDGE_TYPE = "orso:preview";

/** Messages the bridge in the framed page sends back to the app. */
export type BridgeMessage =
  | { type: typeof BRIDGE_TYPE; op: "ready"; version: number; capture: boolean }
  /** `element.screenshot`, when present, is a PNG data URL the page took of the element. */
  | { type: typeof BRIDGE_TYPE; op: "pick"; element: PickedElement }
  | { type: typeof BRIDGE_TYPE; op: "cancel" }
  | { type: typeof BRIDGE_TYPE; op: "path"; url: string }
  | { type: typeof BRIDGE_TYPE; op: "fix"; text: string }
  | { type: typeof BRIDGE_TYPE; op: "snapshot"; url: string; html: string }
  | { type: typeof BRIDGE_TYPE; op: "error"; error: PageError }
  | { type: typeof BRIDGE_TYPE; op: "error-cleared" };

const isString = (value: unknown): value is string => typeof value === "string";
const optionalString = (value: unknown) => value === undefined || isString(value);
const PNG_DATA_URL = /^data:image\/png;base64,[A-Za-z0-9+/=]+$/;

/**
 * Narrow an untrusted `MessageEvent.data` to a bridge message. Callers must also check the
 * event's `source` and `origin`: this only checks the shape.
 */
export function isBridgeMessage(data: unknown): data is BridgeMessage {
  if (!data || typeof data !== "object") return false;
  const message = data as Record<string, unknown>;
  if (message.type !== BRIDGE_TYPE) return false;
  if (message.op === "cancel" || message.op === "error-cleared") return true;
  if (message.op === "ready") return typeof message.version === "number";
  // The page's own address; only its path survives `framePath`, so no origin is trusted.
  if (message.op === "path") return isString(message.url) && /^https?:\/\//i.test(message.url);
  // Cross-origin page HTML for Page details (parent runs readPageMeta / auditPage).
  if (message.op === "snapshot") {
    return (
      isString(message.url) &&
      /^https?:\/\//i.test(message.url) &&
      isString(message.html) &&
      message.html.length > 0
    );
  }
  // Preview-checks "Fix this" — plain-language prompt for the agent outbox.
  if (message.op === "fix") return isString(message.text) && message.text.trim().length > 0;
  if (message.op === "error") {
    const error = message.error as Record<string, unknown> | null;
    return (
      !!error &&
      (error.kind === "build" || error.kind === "runtime") &&
      isString(error.message) &&
      optionalString(error.file) &&
      optionalString(error.stack) &&
      optionalString(error.hint)
    );
  }
  if (message.op !== "pick") return false;
  const element = message.element as Record<string, unknown> | null;
  return (
    !!element &&
    isString(element.label) &&
    isString(element.tag) &&
    isString(element.selector) &&
    isString(element.text) &&
    optionalString(element.file) &&
    optionalString(element.loc) &&
    (element.screenshot === undefined ||
      (isString(element.screenshot) && PNG_DATA_URL.test(element.screenshot)))
  );
}

/** A PNG data URL (from the bridge's self-capture) as a Blob; null when it isn't one. */
export function dataUrlToBlob(dataUrl: string): Blob | null {
  if (!PNG_DATA_URL.test(dataUrl)) return null;
  try {
    const bytes = atob(dataUrl.slice(dataUrl.indexOf(",") + 1));
    const out = new Uint8Array(bytes.length);
    for (let i = 0; i < bytes.length; i++) out[i] = bytes.charCodeAt(i);
    return new Blob([out], { type: "image/png" });
  } catch {
    return null;
  }
}
