/**
 * Public-domain Renaissance paintings used as painted backdrops behind the product previews.
 * Files live in `public/paintings/`; sources and licences are listed in `public/paintings/CREDITS.md`.
 */
export const paintings = {
  patinir: {
    file: "patinir.webp",
    title: "Landscape with the Flight into Egypt",
    artist: "Joachim Patinir",
    date: "c. 1516–17",
  },
  bruegel: {
    file: "bruegel.webp",
    title: "The Harvesters",
    artist: "Pieter Bruegel the Elder",
    date: "1565",
  },
  giorgione: {
    file: "giorgione.webp",
    title: "The Tempest",
    artist: "Giorgione",
    date: "c. 1506–08",
  },
  altdorfer: {
    file: "altdorfer.webp",
    title: "Danube Landscape",
    artist: "Albrecht Altdorfer",
    date: "c. 1520–25",
  },
  leonardo: {
    file: "leonardo.webp",
    title: "The Virgin of the Rocks",
    artist: "Leonardo da Vinci",
    date: "c. 1491–1508",
  },
} as const;

export type PaintingId = keyof typeof paintings;

/** Base-aware URL for a painting in `public/paintings/`. */
export const paintingUrl = (id: PaintingId) =>
  `${import.meta.env.BASE_URL.replace(/\/?$/, "/")}paintings/${paintings[id].file}`;
