#!/usr/bin/env node
// Generates src-tauri/resources/countries.zh.json from radio-browser's country list.
// Node >= 18 (global fetch). No dependencies.
//
// Output: { "AD": "安道尔", "AE": "阿拉伯联合酋长国", ... } sorted by country code.

import { mkdir, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const API = 'https://de1.api.radio-browser.info';
const UA = 'radiobar/0.1';
const OUT = fileURLToPath(new URL('../src-tauri/resources/countries.zh.json', import.meta.url));
const TIMEOUT_MS = 20_000;
const RETRIES = 2;

async function getJson(path) {
  let lastErr;
  for (let attempt = 0; attempt <= RETRIES; attempt++) {
    if (attempt > 0) {
      await new Promise((resolve) => setTimeout(resolve, 500 * attempt));
      console.warn(`  retry ${attempt}/${RETRIES} GET ${path}`);
    }
    try {
      const res = await fetch(`${API}${path}`, {
        headers: { 'User-Agent': UA, Accept: 'application/json' },
        signal: AbortSignal.timeout(TIMEOUT_MS),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText}`);
      return await res.json();
    } catch (err) {
      lastErr = err;
      console.warn(`  failed (attempt ${attempt + 1}/${RETRIES + 1}): ${err.message}`);
    }
  }
  throw lastErr;
}

const displayNames = new Intl.DisplayNames(['zh'], { type: 'region' });
const raw = await getJson('/json/countries');
if (!Array.isArray(raw)) throw new Error('expected an array from /json/countries');

const map = Object.create(null);
let skipped = 0;

for (const entry of raw) {
  const code = String(entry?.iso_3166_1 ?? '').trim().toUpperCase();
  if (!/^[A-Z]{2}$/.test(code)) {
    skipped++;
    continue;
  }
  let name;
  try {
    name = displayNames.of(code);
  } catch {
    name = undefined;
  }
  // Intl returns the code itself for regions it does not know.
  if (!name || name === code) {
    skipped++;
    continue;
  }
  map[code] = name;
}

const sorted = {};
for (const code of Object.keys(map).sort()) sorted[code] = map[code];

const count = Object.keys(sorted).length;
console.log(`countries: ${raw.length} fetched, ${count} mapped, ${skipped} skipped`);
console.log(`  CN=${sorted.CN} JP=${sorted.JP} US=${sorted.US} DE=${sorted.DE} FR=${sorted.FR}`);

if (count < 200) throw new Error(`expected >= 200 countries, got ${count}`);
if (sorted.CN !== '中国') throw new Error(`CN should be 中国, got ${sorted.CN}`);
if (sorted.JP !== '日本') throw new Error(`JP should be 日本, got ${sorted.JP}`);

await mkdir(dirname(OUT), { recursive: true });
const json = `${JSON.stringify(sorted, null, 2)}\n`;
await writeFile(OUT, json, 'utf8');
console.log(`wrote ${OUT} (${count} entries, ${Buffer.byteLength(json)} bytes)`);
