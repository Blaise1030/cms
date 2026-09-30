// Generates the PWA icons (dependency-free PNG writer). Run: node scripts/gen-icons.mjs
import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

const out = path.resolve(import.meta.dirname, "../public/icons");
mkdirSync(out, { recursive: true });

const BG = [15, 15, 18];
const FG = [245, 245, 247];
const ACCENT = [99, 102, 241];

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
  const SS = 3; // supersampling per axis
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

// Mark: a "window" frame with a title bar dot and a content block, inside a safe zone.
// `scale` shrinks the mark so maskable icons keep it inside the 80% safe zone.
function mark(u, v, scale) {
  const x = (u - 0.5) / scale + 0.5;
  const y = (v - 0.5) / scale + 0.5;
  const frame = inRoundRect(x, y, 0.22, 0.26, 0.78, 0.74, 0.07);
  const inner = inRoundRect(x, y, 0.27, 0.38, 0.73, 0.69, 0.03);
  const dot = (x - 0.3) ** 2 + (y - 0.32) ** 2 <= 0.022 ** 2;
  const block = inRoundRect(x, y, 0.33, 0.45, 0.52, 0.62, 0.02);
  if (dot || block) return ACCENT;
  if (frame && !inner) return FG;
  return null;
}

function icon({ maskable }) {
  return (u, v) => {
    const m = mark(u, v, maskable ? 0.8 : 1);
    if (m) return [...m, 1];
    if (maskable || inRoundRect(u, v, 0, 0, 1, 1, 0.22)) return [...BG, 1];
    return [0, 0, 0, 0];
  };
}

const files = {
  "icon-192.png": png(192, icon({ maskable: false })),
  "icon-512.png": png(512, icon({ maskable: false })),
  "maskable-512.png": png(512, icon({ maskable: true })),
  "apple-touch-icon.png": png(180, icon({ maskable: true })),
};
for (const [name, data] of Object.entries(files)) writeFileSync(path.join(out, name), data);
console.log("wrote", Object.keys(files).join(", "));
