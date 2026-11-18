import { getImage } from "astro:assets";
import type { ImageMetadata } from "astro";
import type { AppShotData, AppShotSource } from "./appShotTypes";
import { href } from "./href";

export type { AppShotData } from "./appShotTypes";

// Captured by e2e/scripts/landing-shots.mjs as <id>-<light|dark>.png (2x device pixels).
const files = import.meta.glob<ImageMetadata>("../assets/app/*.png", { eager: true, import: "default" });
// Clips the script recorded for some shots, served as-is from public/app/.
const clips = new Set(
  Object.keys(import.meta.glob("../../public/app/*.webm")).map((path) => path.split("/").pop()!),
);

const WIDTHS = [640, 960, 1280, 1600, 2400];

function file(id: string, scheme: "light" | "dark"): ImageMetadata {
  const image = files[`../assets/app/${id}-${scheme}.png`];
  if (!image) throw new Error(`Missing app shot ${id}-${scheme}.png — run e2e/scripts/landing-shots.mjs`);
  return image;
}

async function variant(image: ImageMetadata): Promise<AppShotSource> {
  const widths = WIDTHS.filter((w) => w <= image.width);
  const result = await getImage({ src: image, format: "webp", quality: 82, widths });
  return { src: result.src, srcSet: result.srcSet.attribute };
}

/** Resolve an app shot pair (and its clip, if any) to optimized, base-aware URLs. */
export async function appShot(id: string): Promise<AppShotData> {
  const lightFile = file(id, "light");
  const [light, dark] = await Promise.all([variant(lightFile), variant(file(id, "dark"))]);
  const hasVideo = clips.has(`${id}-light.webm`) && clips.has(`${id}-dark.webm`);
  return {
    id,
    // Intrinsic size in CSS pixels (shots are captured at deviceScaleFactor 2).
    width: Math.round(lightFile.width / 2),
    height: Math.round(lightFile.height / 2),
    light,
    dark,
    ...(hasVideo ? { video: { light: href(`/app/${id}-light.webm`), dark: href(`/app/${id}-dark.webm`) } } : {}),
  };
}

export const appShots = (ids: readonly string[]) => Promise.all(ids.map(appShot));
