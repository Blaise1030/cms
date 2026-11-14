import { toast } from "vue-sonner";
import type { Router } from "vue-router";
import { errorMessage } from "@orso/utils";
import { projectPath } from "@/router";
import type { usePreviewStore } from "@/modules/preview/store";

export async function openInPreview(opts: {
  projectId: string;
  collection: string;
  id: string;
  resolve: (project: string, collection: string, id: string) => Promise<{ path: string }>;
  preview: ReturnType<typeof usePreviewStore>;
  router: Router;
}) {
  try {
    const { path } = await opts.resolve(opts.projectId, opts.collection, opts.id);
    opts.preview.openPath(opts.projectId, path);
    await opts.router.push(projectPath(opts.projectId, "preview"));
  } catch (e) {
    toast.error(errorMessage(e));
  }
}