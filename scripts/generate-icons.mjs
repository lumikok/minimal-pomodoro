import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const directory = fileURLToPath(new URL("../src-tauri/icons/", import.meta.url));
mkdirSync(directory, { recursive: true });

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, bytes) {
  const name = Buffer.from(type);
  const length = Buffer.alloc(4);
  length.writeUInt32BE(bytes.length);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([name, bytes])));
  return Buffer.concat([length, name, bytes, crc]);
}

function lineDistance(x, y, ax, ay, bx, by) {
  const amount = Math.max(0, Math.min(1, ((x - ax) * (bx - ax) + (y - ay) * (by - ay)) / ((bx - ax) ** 2 + (by - ay) ** 2)));
  return Math.hypot(x - ax - amount * (bx - ax), y - ay - amount * (by - ay));
}

function makePng(size) {
  const rows = Buffer.alloc(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      // Supersampling keeps the transparent tomato silhouette crisp in the tray.
      const sum = [0, 0, 0, 0];
      for (let sy = 0; sy < 4; sy++) for (let sx = 0; sx < 4; sx++) {
        const nx = (x + (sx + 0.5) / 4) / size, ny = (y + (sy + 0.5) / 4) / size;
        let color = [0, 0, 0, 0];
        if (((nx - 0.5) / 0.425) ** 2 + ((ny - 0.565) / 0.38) ** 2 < 1) color = [211, 101, 79, 255];
        if (Math.hypot(nx - 0.5, ny - 0.575) < 0.272) color = [255, 250, 240, 255];
        if (lineDistance(nx, ny, 0.5, 0.575, 0.5, 0.404) < 0.026 || lineDistance(nx, ny, 0.5, 0.575, 0.638, 0.655) < 0.026) color = [80, 110, 88, 255];
        const leaves = [[0.5, 0.225, 0.36, 0.12, 0.31, 0.21], [0.5, 0.225, 0.64, 0.12, 0.69, 0.21], [0.5, 0.245, 0.43, 0.065, 0.57, 0.065]];
        for (const [ax, ay, bx, by, cx, cy] of leaves) {
          const d1 = (nx - bx) * (ay - by) - (ax - bx) * (ny - by);
          const d2 = (nx - cx) * (by - cy) - (bx - cx) * (ny - cy);
          const d3 = (nx - ax) * (cy - ay) - (cx - ax) * (ny - ay);
          if ((d1 >= 0 && d2 >= 0 && d3 >= 0) || (d1 <= 0 && d2 <= 0 && d3 <= 0)) color = [80, 110, 88, 255];
        }
        for (let channel = 0; channel < 4; channel++) sum[channel] += color[channel];
      }
      const color = sum.map(value => Math.round(value / 16));
      if (color[3] > 0) for (let channel = 0; channel < 3; channel++) color[channel] = Math.min(255, Math.round(color[channel] * 255 / color[3]));
      const offset = y * (size * 4 + 1) + 1 + x * 4;
      rows.set(color, offset);
    }
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(size, 0); header.writeUInt32BE(size, 4);
  header[8] = 8; header[9] = 6;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", header), chunk("IDAT", deflateSync(rows)), chunk("IEND", Buffer.alloc(0))]);
}

const sizes = [16, 32, 48, 64, 128, 256];
const images = sizes.map(makePng);
for (let index = 0; index < sizes.length; index++) writeFileSync(`${directory}${sizes[index]}x${sizes[index]}.png`, images[index]);
const icoHeader = Buffer.alloc(6 + sizes.length * 16);
icoHeader.writeUInt16LE(1, 2); icoHeader.writeUInt16LE(sizes.length, 4);
let offset = icoHeader.length;
for (let index = 0; index < sizes.length; index++) {
  const entry = 6 + index * 16;
  icoHeader[entry] = sizes[index] === 256 ? 0 : sizes[index];
  icoHeader[entry + 1] = icoHeader[entry];
  icoHeader.writeUInt16LE(1, entry + 4); icoHeader.writeUInt16LE(32, entry + 6);
  icoHeader.writeUInt32LE(images[index].length, entry + 8); icoHeader.writeUInt32LE(offset, entry + 12);
  offset += images[index].length;
}
writeFileSync(`${directory}icon.ico`, Buffer.concat([icoHeader, ...images]));
console.log("Generated tomato clock icons (16–256 px, transparent PNG and Windows ICO).");
