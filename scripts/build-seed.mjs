#!/usr/bin/env node
// Generates src-tauri/resources/stations.seed.json from radio-browser.
// Node >= 18 (global fetch). No dependencies.
//
// Selection: top 50 stations (by clickcount, broken hidden) for every country that
// radio-browser knows, plus the global top 500. Deduped by stationuuid; entries that
// came from a country list win over the global list.
//
// Output shape:
//   { "generated_at": "<ISO>", "stations": [ { i,n,c,u,o,k,b,h,v,t,x } ] }
//   i=stationuuid n=name c=countrycode u=url_resolved o=url k=codec
//   b=bitrate h=hls v=votes t=tags(first 2) x=clickcount

import { mkdir, stat, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const API = 'https://de1.api.radio-browser.info';
const UA = 'radiobar/0.1';
const OUT = fileURLToPath(new URL('../src-tauri/resources/stations.seed.json', import.meta.url));

const CONCURRENCY = 8;
const RETRIES = 2;
const TIMEOUT_MS = 20_000;
const RETRY_DELAY_MS = 500;
const PER_COUNTRY = 50;
const GLOBAL_TOP = 500;
const MIN_STATIONS = 5_500;
const MAX_STATIONS = 7_000;

async function getJson(path) {
  let lastErr;
  for (let attempt = 0; attempt <= RETRIES; attempt++) {
    if (attempt > 0) {
      await new Promise((resolve) => setTimeout(resolve, RETRY_DELAY_MS * attempt));
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
      console.warn(`  failed (attempt ${attempt + 1}/${RETRIES + 1}) GET ${path}: ${err.message}`);
    }
  }
  throw lastErr;
}

/** Runs fn over items with at most `limit` in flight; preserves result order. */
async function mapLimit(items, limit, fn) {
  const results = new Array(items.length);
  let nextIndex = 0;
  const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (true) {
      const index = nextIndex++;
      if (index >= items.length) return;
      results[index] = await fn(items[index], index);
    }
  });
  await Promise.all(workers);
  return results;
}

function toInt(value) {
  const n = Number(value);
  return Number.isFinite(n) ? Math.trunc(n) : 0;
}

function httpUrl(value) {
  const url = typeof value === 'string' ? value.trim() : '';
  return /^https?:\/\//i.test(url) ? url : '';
}

/** Raw radio-browser station -> compact record. Returns null when unusable. */
function toShort(raw, fallbackCountry) {
  const id = String(raw?.stationuuid ?? '').trim();
  const name = String(raw?.name ?? '').trim();
  if (!id || !name) return null;

  const url = httpUrl(raw.url);
  const resolved = httpUrl(raw.url_resolved);
  const primary = resolved || url;
  const original = url || resolved;
  if (!primary) return null;

  const tags = String(raw.tags ?? '')
    .split(',')
    .map((tag) => tag.trim())
    .filter(Boolean)
    .slice(0, 2);

  const country = (String(raw.countrycode ?? '').trim() || fallbackCountry || '').toUpperCase();

  return {
    i: id,
    n: name,
    c: country,
    u: primary,
    o: original,
    k: String(raw.codec ?? '').trim(),
    b: toInt(raw.bitrate),
    h: raw.hls === true || toInt(raw.hls) === 1,
    v: toInt(raw.votes),
    t: tags,
    x: toInt(raw.clickcount),
  };
}

const startedAt = Date.now();

// --- 1. country list -------------------------------------------------------
const countryList = await getJson('/json/countries');
if (!Array.isArray(countryList)) throw new Error('expected an array from /json/countries');

const codes = [
  ...new Set(
    countryList
      .map((entry) => String(entry?.iso_3166_1 ?? '').trim().toUpperCase())
      .filter((code) => /^[A-Z]{2}$/.test(code)),
  ),
].sort();
console.log(`countries: ${countryList.length} listed, ${codes.length} queries to run`);

// --- 2. per-country top 50 ------------------------------------------------
const perCountry = await mapLimit(codes, CONCURRENCY, async (code) => {
  const path = `/json/stations/bycountrycodeexact/${code}?limit=${PER_COUNTRY}&hidebroken=true&order=clickcount&reverse=true`;
  try {
    const list = await getJson(path);
    if (!Array.isArray(list)) throw new Error('response is not an array');
    return { code, stations: list };
  } catch (err) {
    return { code, error: err.message };
  }
});

const failed = perCountry.filter((result) => result.error);
const countryCount = perCountry.length - failed.length;

// --- 3. global top 500 ----------------------------------------------------
let globalTop = [];
let globalError = null;
try {
  const top = await getJson(`/json/stations/topclick/${GLOBAL_TOP}?hidebroken=true`);
  if (!Array.isArray(top)) throw new Error('response is not an array');
  globalTop = top;
} catch (err) {
  globalError = err.message;
}

// --- 4. merge, dedupe (country lists first), compact ----------------------
const byId = new Map();
let dropped = 0;
let duplicates = 0;

const add = (raw, fallbackCountry) => {
  const station = toShort(raw, fallbackCountry);
  if (!station) {
    dropped++;
    return;
  }
  if (byId.has(station.i)) {
    duplicates++;
    return;
  }
  byId.set(station.i, station);
};

for (const result of perCountry) {
  if (result.error) continue;
  for (const raw of result.stations) add(raw, result.code);
}
for (const raw of globalTop) add(raw, '');

const stations = [...byId.values()].sort(
  (a, b) => b.x - a.x || (a.n < b.n ? -1 : a.n > b.n ? 1 : 0) || (a.i < b.i ? -1 : a.i > b.i ? 1 : 0),
);

// --- 5. build, validate, then write --------------------------------------
const generatedAt = new Date().toISOString();
// One station per line keeps the artifact ~1.5 MB instead of ~5 MB pretty-printed.
const body = stations.map((station) => `    ${JSON.stringify(station)}`).join(',\n');
const json = `{\n  "generated_at": ${JSON.stringify(generatedAt)},\n  "stations": [\n${body}\n  ]\n}\n`;

// Gate before touching the artifact so a degraded run cannot clobber the
// committed seed with a smaller one.
const inRange = stations.length >= MIN_STATIONS && stations.length <= MAX_STATIONS;
let size = 0;
if (inRange) {
  await mkdir(dirname(OUT), { recursive: true });
  await writeFile(OUT, json, 'utf8');
  ({ size } = await stat(OUT));
}

// --- 6. report ------------------------------------------------------------
const elapsed = ((Date.now() - startedAt) / 1000).toFixed(1);
console.log('');
console.log(`countries:        ${countryCount} ok / ${codes.length} (${failed.length} failed)`);
if (failed.length > 0) {
  console.log(`failed countries: ${failed.map((result) => `${result.code} (${result.error})`).join(', ')}`);
}
console.log(`global top ${GLOBAL_TOP}:   ${globalTop.length} fetched${globalError ? ` (FAILED: ${globalError})` : ''}`);
console.log(`stations:         ${stations.length} (${duplicates} duplicate ids skipped, ${dropped} unusable records dropped)`);
if (inRange) console.log(`file:             ${OUT} (${(size / 1024 / 1024).toFixed(2)} MB)`);
console.log(`generated_at:     ${generatedAt}`);
console.log(`elapsed:          ${elapsed}s`);

if (!inRange) {
  console.error(
    `ERROR: station count ${stations.length} is outside the required ${MIN_STATIONS}-${MAX_STATIONS} range; ` +
      'the existing artifact was left untouched',
  );
  process.exitCode = 1;
}
if (globalError) {
  console.error('ERROR: global top list could not be fetched');
  process.exitCode = 1;
}
