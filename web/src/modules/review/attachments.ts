import type { AttachedImage } from "./queries";

/** Images a comment may carry, as the daemon accepts them. */
export const IMAGE_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];
export const MAX_IMAGE_BYTES = 5 * 1024 * 1024;
export const MAX_IMAGES = 10;

/**
 * The `files` that can be attached next to `already` attached ones: images of an accepted
 * type, 5 MB at most, 10 in all. `refused` says why any were left out (null when none were).
 */
export function pickImages(files: File[], already: number): { accepted: File[]; refused: string | null } {
  const reasons = new Set<string>();
  const accepted: File[] = [];
  for (const f of files) {
    if (!IMAGE_TYPES.includes(f.type)) reasons.add("Only PNG, JPEG, GIF and WebP images can be attached.");
    else if (f.size > MAX_IMAGE_BYTES) reasons.add(`${f.name} is larger than 5 MB.`);
    else if (already + accepted.length >= MAX_IMAGES) reasons.add("A message can carry at most 10 images.");
    else accepted.push(f);
  }
  return { accepted, refused: reasons.size ? [...reasons].join(" ") : null };
}

export function readImage(file: File): Promise<AttachedImage> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve({ name: file.name || "image", mime: file.type, dataUrl: String(reader.result) });
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

/**
 * Puts an image on the clipboard to paste elsewhere (on GitHub, when the repository won't store
 * it). Clipboards take PNG, so other types are redrawn as PNG first. False when not allowed.
 */
export async function copyImageToClipboard(dataUrl: string): Promise<boolean> {
  try {
    if (!navigator.clipboard?.write || typeof ClipboardItem === "undefined") return false;
    const blob = await (await fetch(dataUrl)).blob();
    const png = blob.type === "image/png" ? blob : await toPng(blob);
    await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
    return true;
  } catch {
    return false;
  }
}

async function toPng(blob: Blob): Promise<Blob> {
  const bitmap = await createImageBitmap(blob);
  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  canvas.getContext("2d")?.drawImage(bitmap, 0, 0);
  return new Promise((resolve, reject) =>
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("png"))), "image/png"),
  );
}
