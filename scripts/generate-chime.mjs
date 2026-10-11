import { mkdirSync, writeFileSync } from 'node:fs';
// Original two-note chime, followed by silence. No media downloads/dependencies.
const rate = 22050, seconds = 1.6, frames = Math.round(rate * seconds);
const data = Buffer.alloc(frames * 2);
for (let i = 0; i < frames; i++) {
  const t = i / rate;
  let value = 0;
  for (const [start, frequency] of [[0, 660], [0.26, 880]]) {
    const age = t - start;
    if (age >= 0 && age < 0.55) {
      const envelope = Math.min(1, age / 0.015) * Math.exp(-age * 7) * Math.min(1, (0.55 - age) / 0.04);
      value += Math.sin(2 * Math.PI * frequency * age) * envelope * 0.2;
    }
  }
  data.writeInt16LE(Math.round(value * 32767), i * 2);
}
const header = Buffer.alloc(44);
header.write('RIFF'); header.writeUInt32LE(36 + data.length, 4); header.write('WAVEfmt ', 8);
header.writeUInt32LE(16, 16); header.writeUInt16LE(1, 20); header.writeUInt16LE(1, 22);
header.writeUInt32LE(rate, 24); header.writeUInt32LE(rate * 2, 28); header.writeUInt16LE(2, 32);
header.writeUInt16LE(16, 34); header.write('data', 36); header.writeUInt32LE(data.length, 40);
mkdirSync('src-tauri/sounds', { recursive: true });
writeFileSync('src-tauri/sounds/chime.wav', Buffer.concat([header, data]));
