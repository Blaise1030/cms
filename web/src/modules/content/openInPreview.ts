import { toast } from "vue-sonner";
import { errorMessage, frameHref } from "@orso/utils";

/** Resolve the entry's site path and open the live preview URL in a new tab. */
export async function openInPreview(opts: {
  projectId: string;
  collection: string;
  id: string;
  resolve: (project: string, collection: string, id: string) => Promise<{ path: string }>;
  /** Project preview origin, e.g. `{origin}/preview/{id}/`. */
  previewBaseUrl: string;
  open?: (url: string, target?: string, features?: string) => Window | null;
}) {
  try {
    const { path } = await opts.resolve(opts.projectId, opts.collection, opts.id);
    const href = new URL(frameHref(path, new URL(opts.previewBaseUrl).pathname), opts.previewBaseUrl).href;
    (opts.open ?? window.open)(href, "_blank", "noopener");
  } catch (e) {
    toast.error(errorMessage(e));
  }
}
