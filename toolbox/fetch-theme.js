// Fetches the shared yl-theme files (theme/yl-theme.css, theme/yl-theme.js) at the version pinned in theme.json and
// checks their SHA-256, so a changed or corrupted file fails instead of shipping.
//   node fetch-theme.js          fetch what is missing or wrong (npm start / npm run dist do this first)
//   node fetch-theme.js --bump   pin the newest published version and rewrite theme.json
// Also fetches favicon.svg and renders theme/icon.png from it.
// theme/yl-fonts.css is licensed and not published: it stays in the repo.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const BASE = 'https://theme.yannislam.org';
const PIN = path.join(__dirname, 'theme.json');
const DIR = path.join(__dirname, 'theme');
const FILES = ['yl-theme.css', 'yl-theme.js', 'favicon.svg'];
const sha256 = (buf) => crypto.createHash('sha256').update(buf).digest('hex');

async function get(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: ${res.status}`);
  return Buffer.from(await res.arrayBuffer());
}

async function fetchPinned() {
  const { version, files } = JSON.parse(fs.readFileSync(PIN, 'utf8'));
  fs.mkdirSync(DIR, { recursive: true });
  for (const name of FILES) {
    const file = path.join(DIR, name);
    if (fs.existsSync(file) && sha256(fs.readFileSync(file)) === files[name]) continue;
    const data = await get(`${BASE}/${version}/${name}`);
    if (sha256(data) !== files[name]) throw new Error(`${name} at ${version} does not match its hash in theme.json`);
    fs.writeFileSync(file, data);
    console.log(`fetched ${name} (${version})`);
  }
  await renderIcon();
}

// The theme's favicon.ico stops at 48px, but the exe icon needs 256px+, so render the SVG to theme/icon.png
// (used for the exe icon and the window icon).
async function renderIcon() {
  const svg = path.join(DIR, 'favicon.svg'), png = path.join(DIR, 'icon.png');
  if (fs.existsSync(png) && fs.statSync(png).mtimeMs >= fs.statSync(svg).mtimeMs) return;
  await require('sharp')(svg, { density: 384 }).resize(512, 512, { fit: 'contain', background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toFile(png);
  console.log('rendered icon.png');
}

async function bump() {
  const versions = JSON.parse((await get(`${BASE}/versions.json`)).toString());
  const latest = Object.keys(versions).sort().pop(); // v + UTC timestamp sorts as text
  const pin = { version: latest, files: Object.fromEntries(FILES.map((n) => [n, versions[latest][n]])) };
  fs.writeFileSync(PIN, JSON.stringify(pin, null, 2) + '\n');
  console.log(`pinned ${latest}`);
}

(process.argv.includes('--bump') ? bump() : fetchPinned()).catch((e) => { console.error(e.message); process.exit(1); });
