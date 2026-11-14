import { sortFindings, type Finding } from "./findings";
import { readPageMeta, type PageMeta } from "./meta";
import { auditPage } from "./page";
import { resourceSummary, type ResourceSummary } from "./resourceSummary";

export type PageAudit = {
  meta: PageMeta;
  findings: Finding[];
  resources: ResourceSummary;
  path: string;
};

/** Run page meta + rules over a Document (live iframe or DOMParser snapshot). */
export function auditDocument(doc: Document, path: string, pageUrl?: string): PageAudit {
  return {
    meta: readPageMeta(doc, { baseUrl: pageUrl }),
    findings: sortFindings(auditPage(doc, { path })),
    resources: resourceSummary(doc),
    path,
  };
}

/** Parse HTML from the preview bridge and audit it in the parent. */
export function auditHtml(html: string, path: string, pageUrl?: string): PageAudit {
  const doc = new DOMParser().parseFromString(html, "text/html");
  return auditDocument(doc, path, pageUrl);
}
