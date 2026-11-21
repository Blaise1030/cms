import { toast } from "@/lib/toast";
import { type PreviewAccess, previewHref } from "@/core/previewOrigin";
import { errorMessage } from "@orso/utils";

/** Resolve the entry's site path and open it on the live preview in a new tab. */
export async function openInPreview(opts: {
  projectId: string;
  collection: string;
  id: string;
  resolve: (project: string, collection: string, id: string) => Promise<{ path: string }>;
  /** The project's preview origin (`GET /api/projects/{id}/preview`). */
  access: (project: string) => Promise<PreviewAccess>;
  /** The daemon's origin; the preview origin shares its host. */
  daemonOrigin: string;
  open?: (url: string, target?: string, features?: string) => Window | null;
}) {
  try {
    const [{ path }, access] = await Promise.all([
      opts.resolve(opts.projectId, opts.collection, opts.id),
      opts.access(opts.projectId),
    ]);
    // A new tab has no cookie for the preview origin yet: the first load carries its token.
    const href = previewHref(access, opts.daemonOrigin, path, true);
    (opts.open ?? window.open)(href, "_blank", "noopener");
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
