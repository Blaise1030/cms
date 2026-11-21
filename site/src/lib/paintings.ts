/**
 * Figure-free Renaissance landscapes used as painted backdrops behind the product previews.
 * Files live in `public/paintings/`; sources and licences are listed in `public/paintings/CREDITS.md`.
 */
export const paintings = {
  danube: {
    file: "altdorfer-danube.webp",
    width: 1200,
    height: 750,
    title: "Danube Landscape with Wörth Castle",
    artist: "Albrecht Altdorfer",
    date: "c. 1528–30",
  },
  bridge: {
    file: "altdorfer-bridge.webp",
    width: 1200,
    height: 750,
    title: "Landscape with a Footbridge",
    artist: "Albrecht Altdorfer",
    date: "1516",
  },
  toledo: {
    file: "elgreco-toledo.webp",
    width: 1200,
    height: 750,
    title: "View of Toledo",
    artist: "El Greco",
    date: "c. 1598–99",
  },
  alps: {
    file: "verhaecht-alps.webp",
    width: 1200,
    height: 593,
    title: "Alpine Landscape",
    artist: "Tobias Verhaecht",
    date: "c. 1600",
  },
  rocks: {
    file: "valckenborch-rocks.webp",
    width: 1200,
    height: 680,
    title: "Mountainous Landscape",
    artist: "Lucas van Valckenborch",
    date: "1582",
  },
} as const;

export type PaintingId = keyof typeof paintings;

/** Picks a stable painting for a seed (e.g. a post id), so each card keeps the same backdrop. */
export const paintingFor = (seed: string): PaintingId => {
  const ids = Object.keys(paintings) as PaintingId[];
  // FNV-1a: small, stable hash.
  const n = [...seed].reduce((hash, char) => Math.imul(hash ^ char.charCodeAt(0), 16777619) >>> 0, 2166136261);
  return ids[n % ids.length];
};

/** URL of a painting in `public/paintings/`. */
export const paintingUrl = (id: PaintingId) => `/paintings/${paintings[id].file}`;

/** Intrinsic size of a painting, for `width`/`height` attributes. */
export const paintingSize = (id: PaintingId) => {
  const { width, height } = paintings[id];
  return { width, height };
};
