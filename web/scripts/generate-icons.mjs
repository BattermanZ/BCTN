import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { Buffer } from 'node:buffer';
import zlib from 'node:zlib';

function crc32(buf) {
  let crc = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    crc ^= buf[i];
    for (let j = 0; j < 8; j++) {
      const mask = -(crc & 1);
      crc = (crc >>> 1) ^ (0xedb88320 & mask);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const typeBuf = Buffer.from(type, 'ascii');
  const lenBuf = Buffer.alloc(4);
  lenBuf.writeUInt32BE(data.length, 0);
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([lenBuf, typeBuf, data, crcBuf]);
}

function pngRgba(width, height, rgba) {
  const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  ihdr[10] = 0; // compression
  ihdr[11] = 0; // filter
  ihdr[12] = 0; // interlace

  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 0; // filter type 0
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  }
  const idat = zlib.deflateSync(raw, { level: 9 });

  return Buffer.concat([
    signature,
    chunk('IHDR', ihdr),
    chunk('IDAT', idat),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function makeCanvas(size, rgb) {
  const data = new Uint8ClampedArray(size * size * 4);
  for (let i = 0; i < size * size; i++) {
    data[i * 4 + 0] = rgb[0];
    data[i * 4 + 1] = rgb[1];
    data[i * 4 + 2] = rgb[2];
    data[i * 4 + 3] = 255;
  }
  return data;
}

function setPixel(img, size, x, y, rgba) {
  if (x < 0 || y < 0 || x >= size || y >= size) return;
  const idx = (y * size + x) * 4;
  img[idx + 0] = rgba[0];
  img[idx + 1] = rgba[1];
  img[idx + 2] = rgba[2];
  img[idx + 3] = rgba[3] ?? 255;
}

function fillCircle(img, size, cx, cy, r, rgba) {
  const r2 = r * r;
  const x0 = Math.floor(cx - r);
  const x1 = Math.ceil(cx + r);
  const y0 = Math.floor(cy - r);
  const y1 = Math.ceil(cy + r);
  for (let y = y0; y <= y1; y++) {
    for (let x = x0; x <= x1; x++) {
      const dx = x + 0.5 - cx;
      const dy = y + 0.5 - cy;
      if (dx * dx + dy * dy <= r2) setPixel(img, size, x, y, rgba);
    }
  }
}

function fillRing(img, size, cx, cy, rOuter, thickness, rgba) {
  const rInner = Math.max(0, rOuter - thickness);
  const ro2 = rOuter * rOuter;
  const ri2 = rInner * rInner;
  const x0 = Math.floor(cx - rOuter);
  const x1 = Math.ceil(cx + rOuter);
  const y0 = Math.floor(cy - rOuter);
  const y1 = Math.ceil(cy + rOuter);
  for (let y = y0; y <= y1; y++) {
    for (let x = x0; x <= x1; x++) {
      const dx = x + 0.5 - cx;
      const dy = y + 0.5 - cy;
      const d2 = dx * dx + dy * dy;
      if (d2 <= ro2 && d2 >= ri2) setPixel(img, size, x, y, rgba);
    }
  }
}

function fillRoundedRect(img, size, x, y, w, h, r, rgba) {
  const x0 = Math.floor(x);
  const y0 = Math.floor(y);
  const x1 = Math.ceil(x + w);
  const y1 = Math.ceil(y + h);
  for (let py = y0; py < y1; py++) {
    for (let px = x0; px < x1; px++) {
      const rx = px + 0.5;
      const ry = py + 0.5;
      const inCore = rx >= x + r && rx <= x + w - r && ry >= y && ry <= y + h;
      const inCore2 = ry >= y + r && ry <= y + h - r && rx >= x && rx <= x + w;
      if (inCore || inCore2) {
        setPixel(img, size, px, py, rgba);
        continue;
      }

      const corners = [
        [x + r, y + r],
        [x + w - r, y + r],
        [x + r, y + h - r],
        [x + w - r, y + h - r],
      ];
      for (const [cx, cy] of corners) {
        const dx = rx - cx;
        const dy = ry - cy;
        if (dx * dx + dy * dy <= r * r) {
          setPixel(img, size, px, py, rgba);
          break;
        }
      }
    }
  }
}

function fillThickLine(img, size, x0, y0, x1, y1, thickness, rgba) {
  const minX = Math.floor(Math.min(x0, x1) - thickness);
  const maxX = Math.ceil(Math.max(x0, x1) + thickness);
  const minY = Math.floor(Math.min(y0, y1) - thickness);
  const maxY = Math.ceil(Math.max(y0, y1) + thickness);
  const dx = x1 - x0;
  const dy = y1 - y0;
  const len2 = dx * dx + dy * dy || 1;
  const r2 = (thickness / 2) * (thickness / 2);

  for (let y = minY; y <= maxY; y++) {
    for (let x = minX; x <= maxX; x++) {
      const px = x + 0.5;
      const py = y + 0.5;
      const t = ((px - x0) * dx + (py - y0) * dy) / len2;
      const tt = Math.max(0, Math.min(1, t));
      const qx = x0 + tt * dx;
      const qy = y0 + tt * dy;
      const ddx = px - qx;
      const ddy = py - qy;
      if (ddx * ddx + ddy * ddy <= r2) setPixel(img, size, x, y, rgba);
    }
  }
}

function drawDiaperPinIcon(size) {
  const bg = [238, 242, 255]; // indigo-50-ish
  const pin = [79, 70, 229, 255]; // indigo-600
  const white = [255, 255, 255, 255];
  const soft = [243, 244, 246, 255]; // zinc-100

  const img = makeCanvas(size, bg);
  const s = size / 512;

  // Card-like rounded background inset (gives iOS mask some padding).
  fillRoundedRect(img, size, 32 * s, 32 * s, size - 64 * s, size - 64 * s, 96 * s, white);

  // Soft shadow-ish band.
  fillRoundedRect(img, size, 42 * s, 52 * s, size - 84 * s, size - 104 * s, 90 * s, soft);

  // Stylized safety pin: ring + head + needle.
  const cx = 256 * s;
  const cy = 244 * s;
  fillRing(img, size, cx, cy, 150 * s, 22 * s, pin);
  fillCircle(img, size, (256 - 118) * s, (244 - 118) * s, 24 * s, pin);
  fillThickLine(
    img,
    size,
    (256 + 70) * s,
    (244 + 120) * s,
    (256 + 170) * s,
    (244 + 205) * s,
    18 * s,
    pin
  );

  // Simple diaper glyph inside the ring.
  const dx0 = (256 - 74) * s;
  const dy0 = (244 - 35) * s;
  fillRoundedRect(img, size, dx0, dy0, 148 * s, 120 * s, 22 * s, white);
  fillRoundedRect(img, size, (256 - 88) * s, (244 - 18) * s, 40 * s, 42 * s, 16 * s, bg);
  fillRoundedRect(img, size, (256 + 48) * s, (244 - 18) * s, 40 * s, 42 * s, 16 * s, bg);
  fillRoundedRect(img, size, (256 - 60) * s, (244 + 40) * s, 120 * s, 26 * s, 13 * s, bg);

  return Buffer.from(img);
}

function writeIcon(outPath, size) {
  const rgba = drawDiaperPinIcon(size);
  const png = pngRgba(size, size, rgba);
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, png);
}

const root = path.resolve(process.cwd(), 'web', 'static');
writeIcon(path.join(root, 'icons', 'icon-192.png'), 192);
writeIcon(path.join(root, 'icons', 'icon-512.png'), 512);
writeIcon(path.join(root, 'apple-touch-icon.png'), 180);
writeIcon(path.join(root, 'favicon-32.png'), 32);

console.log('icons_generated');
