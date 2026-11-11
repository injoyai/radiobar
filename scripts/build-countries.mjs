#!/usr/bin/env node
// Generates src-tauri/resources/countries.zh.json and countries.en.json from
// radio-browser's country list. Node >= 18 (global fetch). No dependencies.
//
// Output (flat, sorted by country code, 2-space indented):
//   { "AD": "安道尔", "AE": "阿拉伯联合酋长国", ... }        // zh
//   { "AD": "Andorra", "AE": "United Arab Emirates", ... }  // en

import { mkdir, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const API = 'https://de1.api.radio-browser.info';
const UA = 'radiobar/0.1';
const TIMEOUT_MS = 20_000;
const RETRIES = 2;

const OUT_ZH = fileURLToPath(new URL('../src-tauri/resources/countries.zh.json', import.meta.url));
const OUT_EN = fileURLToPath(new URL('../src-tauri/resources/countries.en.json', import.meta.url));

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

function displayName(displayNames, code) {
  try {
    const name = displayNames.of(code);
    // Intl echoes the code back for regions it does not know.
    return name && name !== code ? name : undefined;
  } catch {
    return undefined;
  }
}

const zhNames = new Intl.DisplayNames(['zh'], { type: 'region' });
const enNames = new Intl.DisplayNames(['en'], { type: 'region' });

const raw = await getJson('/json/countries');
if (!Array.isArray(raw)) throw new Error('expected an array from /json/countries');

const zh = Object.create(null);
const en = Object.create(null);
let skipped = 0;

for (const entry of raw) {
  const code = String(entry?.iso_3166_1 ?? '').trim().toUpperCase();
  if (!/^[A-Z]{2}$/.test(code)) {
    skipped++;
    continue;
  }
  const zhName = displayName(zhNames, code);
  if (!zhName) {
    skipped++;
    continue;
  }
  zh[code] = zhName;
  en[code] = displayName(enNames, code) ?? code;
}

const sortedZh = {};
for (const code of Object.keys(zh).sort()) sortedZh[code] = zh[code];
const sortedEn = {};
for (const code of Object.keys(en).sort()) sortedEn[code] = en[code];

const zhCount = Object.keys(sortedZh).length;
const enCount = Object.keys(sortedEn).length;
console.log(`countries: ${raw.length} fetched, ${zhCount} zh / ${enCount} en mapped, ${skipped} skipped`);
console.log(`  CN=${sortedZh.CN} JP=${sortedZh.JP} US=${sortedZh.US} DE=${sortedZh.DE} FR=${sortedZh.FR}`);

// Validate before touching the artifacts so a degraded run cannot clobber them.
if (zhCount < 200) throw new Error(`expected >= 200 zh entries, got ${zhCount}`);
if (zhCount !== enCount) throw new Error(`zh/en entry count mismatch: ${zhCount} vs ${enCount}`);
if (sortedZh.CN !== '中国') throw new Error(`CN should be 中国, got ${sortedZh.CN}`);
if (sortedZh.JP !== '日本') throw new Error(`JP should be 日本, got ${sortedZh.JP}`);

await mkdir(dirname(OUT_ZH), { recursive: true });

const zhJson = `${JSON.stringify(sortedZh, null, 2)}\n`;
await writeFile(OUT_ZH, zhJson, 'utf8');
console.log(`wrote ${OUT_ZH} (${zhCount} entries, ${Buffer.byteLength(zhJson)} bytes)`);

const enJson = `${JSON.stringify(sortedEn, null, 2)}\n`;
await writeFile(OUT_EN, enJson, 'utf8');
console.log(`wrote ${OUT_EN} (${enCount} entries, ${Buffer.byteLength(enJson)} bytes)`);
