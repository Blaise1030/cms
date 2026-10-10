import { css } from "@codemirror/lang-css";
import { html } from "@codemirror/lang-html";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { xml } from "@codemirror/lang-xml";
import { yaml } from "@codemirror/lang-yaml";
import type { Extension } from "@codemirror/state";

/** CodeMirror language support for a project file path (best-effort). */
export function languageForPath(path: string): Extension | null {
  const name = path.split("/").pop() ?? path;
  if (name === "Dockerfile") return null;
  const ext = name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
  switch (ext) {
    case "js":
    case "mjs":
    case "cjs":
      return javascript();
    case "jsx":
      return javascript({ jsx: true });
    case "ts":
    case "mts":
    case "cts":
      return javascript({ typescript: true });
    case "tsx":
      return javascript({ typescript: true, jsx: true });
    case "vue":
    case "astro":
    case "html":
    case "svelte":
      return html();
    case "css":
    case "scss":
      return css();
    case "json":
      return json();
    case "md":
    case "mdx":
      return markdown();
    case "yml":
    case "yaml":
      return yaml();
    case "py":
      return python();
    case "rs":
      return rust();
    case "svg":
    case "xml":
      return xml();
    default:
      return null;
  }
}
