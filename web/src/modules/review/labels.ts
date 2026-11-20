/** Background and readable text colour for a GitHub label colour (hex, no `#`). */
export function labelStyle(color: string): { backgroundColor: string; color: string } {
  const hex = /^[0-9a-f]{6}$/i.test(color) ? color : "8b949e";
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255) as [number, number, number];
  // Relative luminance (sRGB weights, no gamma: close enough to pick dark or light text).
  const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return { backgroundColor: `#${hex}`, color: luminance > 0.55 ? "#1f2328" : "#ffffff" };
}
