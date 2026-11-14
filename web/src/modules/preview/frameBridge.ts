import type { PickedElement } from "./elementPicker";

/** Tag on every `postMessage` between the app and the injected preview bridge. */
export const BRIDGE_TYPE = "orso:preview";

/** Messages the bridge in the framed page sends back to the app. */
export type BridgeMessage =
  | { type: typeof BRIDGE_TYPE; op: "pick"; element: PickedElement }
  | { type: typeof BRIDGE_TYPE; op: "cancel" }
  | { type: typeof BRIDGE_TYPE; op: "path"; url: string }
  | { type: typeof BRIDGE_TYPE; op: "fix"; text: string };

const isString = (value: unknown): value is string => typeof value === "string";

/** Narrow an untrusted `MessageEvent.data` to a bridge message. */
export function isBridgeMessage(data: unknown): data is BridgeMessage {
  if (!data || typeof data !== "object") return false;
  const message = data as Record<string, unknown>;
  if (message.type !== BRIDGE_TYPE) return false;
  if (message.op === "cancel") return true;
  // The page's own address; only its path survives `framePath`, so no origin is trusted.
  if (message.op === "path") return isString(message.url) && /^https?:\/\//i.test(message.url);
  // Preview-checks "Fix this" — plain-language prompt for the agent outbox.
  if (message.op === "fix") return isString(message.text) && message.text.trim().length > 0;
  if (message.op !== "pick") return false;
  const element = message.element as Record<string, unknown> | null;
  return (
    !!element &&
    isString(element.label) &&
    isString(element.tag) &&
    isString(element.selector) &&
    isString(element.text) &&
    (element.file === undefined || isString(element.file)) &&
    (element.loc === undefined || isString(element.loc))
  );
}
