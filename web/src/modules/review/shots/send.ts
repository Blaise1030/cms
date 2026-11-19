import type { ReviewWithShots } from "@/core/types";
import type { ShotItem } from "./queue";

/** Queued screenshot comments as the daemon takes them. */
export function shotItemsForReview(items: ShotItem[]): ReviewWithShots["items"] {
  return items.map((i) => {
    const comma = i.imageDataUrl?.indexOf(",") ?? -1;
    const mime = i.imageDataUrl?.slice(5, i.imageDataUrl.indexOf(";")) || undefined;
    return {
      note: i.note,
      path: i.path,
      side: i.side,
      viewport: i.viewport,
      ...(i.imageDataUrl && comma > 0 ? { imageBase64: i.imageDataUrl.slice(comma + 1), mime } : {}),
    };
  });
}
