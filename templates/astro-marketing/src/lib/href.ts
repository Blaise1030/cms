/** Prefix in-page anchors with the site base so links work under `PUBLIC_BASE`. */
export const href = (h: string) =>
  h.startsWith("#") ? `${import.meta.env.BASE_URL}${h}` : h;
