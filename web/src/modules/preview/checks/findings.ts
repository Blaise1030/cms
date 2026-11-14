export type Severity = "error" | "warning";

export type Finding = {
  rule: string;
  severity: Severity;
  page: string;
  message: string;
  detail?: string;
  count?: number;
};

const SEVERITY_RANK: Record<Severity, number> = { error: 0, warning: 1 };

export function createFinding(
  rule: string,
  severity: Severity,
  page: string,
  message: string,
  extra: { detail?: string; count?: number } = {},
): Finding {
  return { rule, severity, page, message, ...extra };
}

export function sortFindings(findings: Finding[]): Finding[] {
  return [...findings].sort((a, b) => {
    const sd = SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity];
    if (sd !== 0) return sd;
    return a.rule.localeCompare(b.rule);
  });
}
