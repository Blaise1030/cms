// UMD bundles (e.g. @xterm/addon-fit) expect `self` at import time in Node tests.
if (typeof globalThis.self === "undefined") {
  Object.defineProperty(globalThis, "self", { value: globalThis, configurable: true });
}
