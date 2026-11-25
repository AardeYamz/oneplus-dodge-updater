#!/usr/bin/env node
// CI bump entry point for yl-theme's "Bump consumers" job: `node toolbox/fetch-theme.js --bump` pins the newest
// published theme version in toolbox/theme.json. Same result as `scripts\build-toolbox.ps1 -BumpTheme`; keep the two in step.
const fs = require('fs');
const path = require('path');

const BASE = 'https://theme.yannislam.org';
const PIN = path.join(__dirname, 'theme.json');
const FILES = ['yl-theme.css', 'yl-theme.js', 'favicon.svg'];

async function main() {
  if (!process.argv.includes('--bump')) throw new Error('usage: node toolbox/fetch-theme.js --bump');
  const res = await fetch(BASE + '/versions.json');
  if (!res.ok) throw new Error(`${BASE}/versions.json: ${res.status}`);
  const versions = await res.json();
  const latest = Object.keys(versions).sort().pop(); // v + UTC timestamp sorts as text
  const files = Object.fromEntries(FILES.map(n => [n, versions[latest][n]]));
  fs.writeFileSync(PIN, JSON.stringify({ version: latest, files }, null, 2) + '\n');
  console.log('pinned ' + latest);
}
main().catch(e => { console.error(e.message); process.exit(1); });
