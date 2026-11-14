/** @typedef {"error" | "warning"} Severity */

/**
 * @typedef {object} Finding
 * @property {string} rule
 * @property {Severity} severity
 * @property {string} page
 * @property {string} message
 * @property {string} [detail]
 * @property {number} [count]
 */

const SEVERITY_RANK = { error: 0, warning: 1 };

/**
 * @param {string} rule
 * @param {Severity} severity
 * @param {string} page
 * @param {string} message
 * @param {{ detail?: string, count?: number }} [extra]
 * @returns {Finding}
 */
export function createFinding(rule, severity, page, message, extra = {}) {
  return { rule, severity, page, message, ...extra };
}

/**
 * @param {Finding[]} findings
 * @returns {Finding[]}
 */
export function sortFindings(findings) {
  return [...findings].sort((a, b) => {
    const sd = SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity];
    if (sd !== 0) return sd;
    return a.rule.localeCompare(b.rule);
  });
}