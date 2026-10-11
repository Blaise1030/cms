/** Resolve local routes and home-page anchors under the configured site base. */
export const href = (value: string) => {
  const base = import.meta.env.BASE_URL.replace(/\/?$/, "/");
  if (value.startsWith("#")) return `${base}${value}`;
  if (value.startsWith("/") && !value.startsWith("//")) return `${base}${value.slice(1)}`;
  return value;
};
