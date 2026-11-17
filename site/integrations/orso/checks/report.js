import { sortFindings } from "./findings.js";

/**
 * @param {import("./findings.js").Finding[]} findings
 * @returns {string}
 */
export function formatReport(findings) {
  const sorted = sortFindings(findings);
  if (sorted.length === 0) {
    return "Preview checks: no issues found.";
  }

  const lines = ["Preview checks:", ""];
  for (const f of sorted) {
    const tag = f.severity === "error" ? "ERROR" : "warn";
    let line = `[${tag}] ${f.page} — ${f.message}`;
    if (f.detail) line += ` (${f.detail})`;
    lines.push(line);
  }
  return lines.join("\n");
}

/**
 * @param {import("./findings.js").Finding[]} findings
 * @returns {0 | 1}
 */
export function exitCode(findings) {
  return findings.some((f) => f.severity === "error") ? 1 : 0;
}