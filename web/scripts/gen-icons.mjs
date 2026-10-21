// Generates the app icons (dependency-free PNG writer).
// Run: node scripts/gen-icons.mjs
// Writes the PWA icons to ../public/icons and the Tauri icon set to ../../src-tauri/icons.
import { deflateSync } from "node:zlib";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const webIcons = path.resolve(import.meta.dirname, "../public/icons");
const tauriIcons = path.resolve(import.meta.dirname, "../../src-tauri/icons");
mkdirSync(webIcons, { recursive: true });
mkdirSync(tauriIcons, { recursive: true });

const BG = [15, 15, 18];
const FG = [245, 245, 247];

function crc32(buf) {
  let c,
    crc = ~0;
  for (let n = 0; n < buf.length; n++) {
    c = (crc ^ buf[n]) & 0xff;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    crc = (crc >>> 8) ^ c;
  }
  return ~crc >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

function png(size, pixel) {
  const raw = Buffer.alloc((size * 4 + 1) * size);
  // Large icons keep 2x2 samples per pixel; the wall clock cost is quadratic in size.
  const SS = size > 512 ? 2 : 3;
  for (let y = 0; y < size; y++) {
    raw[y * (size * 4 + 1)] = 0;
    for (let x = 0; x < size; x++) {
      let r = 0,
        g = 0,
        b = 0,
        a = 0;
      for (let sy = 0; sy < SS; sy++)
        for (let sx = 0; sx < SS; sx++) {
          const p = pixel((x + (sx + 0.5) / SS) / size, (y + (sy + 0.5) / SS) / size);
          r += p[0] * p[3];
          g += p[1] * p[3];
          b += p[2] * p[3];
          a += p[3];
        }
      const n = SS * SS;
      const o = y * (size * 4 + 1) + 1 + x * 4;
      raw[o] = a ? Math.round(r / a) : 0;
      raw[o + 1] = a ? Math.round(g / a) : 0;
      raw[o + 2] = a ? Math.round(b / a) : 0;
      raw[o + 3] = Math.round((a / n) * 255);
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function inRoundRect(x, y, x0, y0, x1, y1, r) {
  if (x < x0 || x > x1 || y < y0 || y > y1) return false;
  const cx = Math.min(Math.max(x, x0 + r), x1 - r);
  const cy = Math.min(Math.max(y, y0 + r), y1 - r);
  return (x - cx) ** 2 + (y - cy) ** 2 <= r * r;
}

// Mark: Lucide's "paw-print" (ISC, https://lucide.dev/icons/paw-print) on its 24 unit grid.
// The icon is stroke-only, so the mark is every point within half a stroke width of the path.
const STROKE = 2;
const TOES = [
  [11, 4, 2],
  [18, 8, 2],
  [20, 16, 2],
];

// Sample an SVG circular arc (large-arc-flag 0, sweep-flag 1, so always the short way round).
function arc(from, to, r, steps) {
  const [x1, y1] = from;
  const [x2, y2] = to;
  const mx = (x1 + x2) / 2;
  const my = (y1 + y2) / 2;
  const dx = (x2 - x1) / 2;
  const dy = (y2 - y1) / 2;
  const half = Math.hypot(dx, dy);
  const h = Math.sqrt(Math.max(0, r * r - half * half));
  const ux = -dy / half;
  const uy = dx / half;
  const centres = [
    [mx + ux * h, my + uy * h],
    [mx - ux * h, my - uy * h],
  ];
  const angleTo = (c, x, y) => Math.atan2(y - c[1], x - c[0]);
  const sweepAngle = (c) => {
    let d = (angleTo(c, x2, y2) - angleTo(c, x1, y1)) % (2 * Math.PI);
    if (d < 0) d += 2 * Math.PI;
    return d;
  };
  const centre = sweepAngle(centres[0]) <= Math.PI ? centres[0] : centres[1];
  const start = angleTo(centre, x1, y1);
  const delta = sweepAngle(centre);
  const points = [];
  for (let i = 1; i <= steps; i++) {
    const t = start + (delta * i) / steps;
    points.push([centre[0] + r * Math.cos(t), centre[1] + r * Math.sin(t)]);
  }
  points[points.length - 1] = [x2, y2];
  return points;
}

function quad(from, control, to, steps) {
  const points = [];
  for (let i = 1; i <= steps; i++) {
    const t = i / steps;
    const u = 1 - t;
    points.push([
      u * u * from[0] + 2 * u * t * control[0] + t * t * to[0],
      u * u * from[1] + 2 * u * t * control[1] + t * t * to[1],
    ]);
  }
  return points;
}

// The pad's outline, flattened: M9 10a5 5 0 0 1 5 5v3.5a3.5 3.5 0 0 1-6.84 1.045
// Q6.52 17.48 4.46 16.84A3.5 3.5 0 0 1 5.5 10Z
const PAD = [
  [9, 10],
  ...arc([9, 10], [14, 15], 5, 20),
  [14, 18.5],
  ...arc([14, 18.5], [7.16, 19.545], 3.5, 20),
  ...quad([7.16, 19.545], [6.52, 17.48], [4.46, 16.84], 12),
  ...arc([4.46, 16.84], [5.5, 10], 3.5, 20),
  [9, 10],
];
const PAD_SEGMENTS = PAD.slice(0, -1)
  .map(([x1, y1], i) => [x1, y1, ...PAD[i + 1]])
  .filter(([x1, y1, x2, y2]) => x1 !== x2 || y1 !== y2);

const PAW_BOUNDS = (() => {
  const xs = [];
  const ys = [];
  const include = (x, y) => {
    xs.push(x - STROKE / 2, x + STROKE / 2);
    ys.push(y - STROKE / 2, y + STROKE / 2);
  };
  for (const [cx, cy, r] of TOES) {
    include(cx - r, cy - r);
    include(cx + r, cy + r);
  }
  for (const [x, y] of PAD) include(x, y);
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const minY = Math.min(...ys);
  const maxY = Math.max(...ys);
  return { minX, maxX, minY, maxY, cx: (minX + maxX) / 2, cy: (minY + maxY) / 2 };
})();

// `--bounds` prints the 24-unit box the mark covers, for framing the standalone SVGs.
if (process.argv.includes("--bounds")) {
  console.log(JSON.stringify(PAW_BOUNDS));
  process.exit(0);
}

function onStroke(x, y) {
  const reach = STROKE / 2;
  for (const [cx, cy, r] of TOES) if (Math.abs(Math.hypot(x - cx, y - cy) - r) <= reach) return true;
  for (const [x1, y1, x2, y2] of PAD_SEGMENTS) {
    const dx = x2 - x1;
    const dy = y2 - y1;
    const t = Math.max(0, Math.min(1, ((x - x1) * dx + (y - y1) * dy) / (dx * dx + dy * dy)));
    if (Math.hypot(x - (x1 + t * dx), y - (y1 + t * dy)) <= reach) return true;
  }
  return false;
}

// `fill` is the share of the icon the paw's longest side covers.
function inPaw(u, v, fill) {
  const span = Math.max(PAW_BOUNDS.maxX - PAW_BOUNDS.minX, PAW_BOUNDS.maxY - PAW_BOUNDS.minY);
  const size = span / fill;
  return onStroke(PAW_BOUNDS.cx + (u - 0.5) * size, PAW_BOUNDS.cy + (v - 0.5) * size);
}

// A paw inside a rounded square; `maskable` bleeds to the edges and keeps the paw in the
// 80% safe zone.
function icon({ maskable }) {
  return (u, v) => {
    if (inPaw(u, v, maskable ? 0.5 : 0.62)) return [...FG, 1];
    if (maskable || inRoundRect(u, v, 0, 0, 1, 1, 0.22)) return [...BG, 1];
    return [0, 0, 0, 0];
  };
}

const raster = (() => {
  const cache = new Map();
  return (size, maskable = false) => {
    const key = `${size}:${maskable}`;
    if (!cache.has(key)) cache.set(key, png(size, icon({ maskable })));
    return cache.get(key);
  };
})();

const pwaFiles = {
  "icon-192.png": raster(192),
  "icon-512.png": raster(512),
  "maskable-512.png": raster(512, true),
  "apple-touch-icon.png": raster(180, true),
};
for (const [name, data] of Object.entries(pwaFiles)) writeFileSync(path.join(webIcons, name), data);

const appFiles = {
  "32x32.png": raster(32),
  "64x64.png": raster(64),
  "128x128.png": raster(128),
  "128x128@2x.png": raster(256),
  "icon.png": raster(512),
};
for (const [name, data] of Object.entries(appFiles)) writeFileSync(path.join(tauriIcons, name), data);

// macOS reads .icns; iconutil assembles it from a standard .iconset folder.
const ICONSET = [
  ["icon_16x16.png", 16],
  ["icon_16x16@2x.png", 32],
  ["icon_32x32.png", 32],
  ["icon_32x32@2x.png", 64],
  ["icon_128x128.png", 128],
  ["icon_128x128@2x.png", 256],
  ["icon_256x256.png", 256],
  ["icon_256x256@2x.png", 512],
  ["icon_512x512.png", 512],
  ["icon_512x512@2x.png", 1024],
];
if (process.platform === "darwin") {
  const dir = mkdtempSync(path.join(tmpdir(), "orso-iconset-"));
  const iconset = `${dir}/orso.iconset`;
  mkdirSync(iconset);
  for (const [name, size] of ICONSET) writeFileSync(path.join(iconset, name), raster(size));
  execFileSync("iconutil", ["-c", "icns", iconset, "-o", path.join(tauriIcons, "icon.icns")]);
  rmSync(dir, { recursive: true });
} else {
  console.warn("Skipping icon.icns: iconutil needs macOS.");
}

const wrote = [...Object.keys(pwaFiles), ...Object.keys(appFiles)];
if (process.platform === "darwin") wrote.push("icon.icns");
console.log("wrote", wrote.join(", "));
