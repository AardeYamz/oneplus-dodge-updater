// Dodge Toolbox: Electron front end for docs/flash-instructions.md / scripts/update-lineage.ps1.
// The main process owns everything that touches the phone, the network or the disk.
const { app, BrowserWindow, ipcMain, dialog, net, shell } = require('electron');
const { spawn } = require('child_process');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const { Readable } = require('stream');
const { pipeline } = require('stream/promises');

const settingsFile = () => path.join(app.getPath('userData'), 'settings.json');
const defaults = {
  // Repo root: the folder that holds scripts\ and artifacts\ (one up from toolbox\ when run from source).
  root: app.isPackaged ? path.dirname(process.env.PORTABLE_EXECUTABLE_FILE || process.execPath) : path.resolve(__dirname, '..'),
  platformTools: 'C:\\Program Files\\platform-tools',
  device: 'dodge',
  gapps: 'artifacts\\mindthegapps\\MindTheGapps-17.0.0-arm64-20261006_111038.zip',
  foxZip: 'artifacts\\orangefox\\OrangeFox-R12.0_2-dodge.zip',
  kmi: 'android15-6.6',
};

function loadSettings() {
  try { return { ...defaults, ...JSON.parse(fs.readFileSync(settingsFile(), 'utf8')) }; } catch { return { ...defaults }; }
}
let settings;
let win;
let running = null; // the child process currently streaming to the log
let cancelled = false; // set by Cancel; checked by the wait loops
let downloading = null; // AbortController for the current download

const log = (text, kind = 'out') => win?.webContents.send('log', { text, kind });
const abs = (p) => (path.isAbsolute(p) ? p : path.join(settings.root, p));
const artifact = (...p) => abs(path.join('artifacts', ...p)); // downloads and flash images live under artifacts\

// Prefer adb/fastboot from the chosen platform-tools folder; the old Essential ADB (1.0.39) on PATH fails on 3 GB sideloads.
function tool(name) {
  const exe = path.join(settings.platformTools, process.platform === 'win32' ? `${name}.exe` : name);
  return fs.existsSync(exe) ? exe : name;
}

// Run a command, stream its output to the log, resolve with { code, out }.
function run(cmd, args, { quiet = false } = {}) {
  return new Promise((resolve) => {
    if (!quiet) log(`> ${path.basename(cmd)} ${args.join(' ')}`, 'cmd');
    let out = '';
    const child = spawn(cmd, args, { windowsHide: true });
    if (!quiet) running = child;
    const onData = (d) => { out += d; if (!quiet) log(d.toString()); };
    child.stdout.on('data', onData);
    child.stderr.on('data', onData);
    child.on('error', (e) => { if (!quiet) log(e.message, 'err'); resolve({ code: -1, out: e.message }); });
    child.on('close', (code) => { if (running === child) running = null; resolve({ code, out }); });
  });
}

const adb = (args, o) => run(tool('adb'), args, o);
const fastboot = (args, o) => run(tool('fastboot'), args, o);
async function must(p) {
  const r = await p;
  if (r.code !== 0) throw new Error(`Command failed (exit ${r.code}).`);
  return r.out;
}

// --- Device state ---
async function deviceState() {
  const a = await adb(['devices'], { quiet: true });
  const f = await fastboot(['devices'], { quiet: true });
  const m = a.code === 0 && a.out.match(/^\S+\t(\w+)\s*$/m);
  if (m) return m[1]; // device | sideload | recovery | unauthorized
  if (f.code === 0 && /^\S+\s+fastboot\s*$/m.test(f.out)) return 'fastboot';
  return a.code === 0 ? 'none' : 'no adb';
}

async function waitFor(state, timeoutSec = 180) {
  log(`Waiting for the phone in '${state}' mode...`, 'info');
  for (let i = 0; i < timeoutSec / 3; i++) {
    if (cancelled) throw new Error('Cancelled.');
    if ((await deviceState()) === state) return;
    await new Promise((r) => setTimeout(r, 3000));
  }
  throw new Error(`Phone never reached '${state}' mode.`);
}

async function waitBoot() {
  await waitFor('device', 900);
  log('Waiting for boot to complete...', 'info');
  for (;;) {
    if (cancelled) throw new Error('Cancelled.');
    const r = await adb(['shell', 'getprop', 'sys.boot_completed'], { quiet: true });
    if (r.out.trim() === '1') return;
    await new Promise((r) => setTimeout(r, 5000));
  }
}

const prop = async (name) => (await adb(['shell', 'getprop', name], { quiet: true })).out.trim();

// --- Downloads ---
async function download(url, out, label) {
  if (fs.existsSync(out)) { log(`Already have ${path.basename(out)}`, 'info'); return; }
  log(`Downloading ${url}`, 'info');
  downloading = new AbortController();
  const res = await net.fetch(url, { signal: downloading.signal });
  if (!res.ok) throw new Error(`Download failed (${res.status}): ${url}`);
  const total = Number(res.headers.get('content-length')) || 0;
  let done = 0;
  let last = 0;
  const body = Readable.fromWeb(res.body);
  body.on('data', (c) => {
    done += c.length;
    if (Date.now() - last < 250) return; // keep IPC traffic down on multi-GB files
    last = Date.now();
    win?.webContents.send('progress', { label, done, total });
  });
  const tmp = `${out}.part`;
  await pipeline(body, fs.createWriteStream(tmp), { signal: downloading.signal });
  fs.renameSync(tmp, out);
  win?.webContents.send('progress', null);
}

async function sha256(file) {
  const h = crypto.createHash('sha256');
  await pipeline(fs.createReadStream(file), h);
  return h.digest('hex');
}

const sizeIs = (file, bytes) => fs.existsSync(file) && fs.statSync(file).size === bytes;
const RECOVERY_SIZE = 104857600;

// --- Build selection (shared between steps) ---
let target = null; // { rel, major, date, dir, zip }

async function latest(build) {
  const res = await net.fetch(`https://download.lineageos.org/api/v1/${settings.device}/nightly/x`);
  const api = await res.json();
  const list = api.response.slice().sort((a, b) => b.datetime - a.datetime);
  const rel = build ? list.find((r) => r.filename.includes(`-${build}-`)) : list[0];
  if (!rel) throw new Error('Build not found in the API response.');
  const [, major, , date] = rel.filename.match(/lineage-(\d+)\.(\d+)-(\d{8})/);
  target = { rel, major, date, dir: artifact('builds', `${major}-${date}`), zip: artifact('lineage', rel.filename) };
  return target;
}

function needTarget() {
  if (!target) throw new Error('Pick a build first (step 1).');
  return target;
}

// --- Steps. Each returns a short result string for the UI. ---
const steps = {
  async status() {
    const state = await deviceState();
    const info = { state };
    if (state === 'device') {
      info.version = await prop('ro.lineage.version');
      info.kernel = (await adb(['shell', 'uname', '-r'], { quiet: true })).out.trim();
      info.slot = await prop('ro.boot.slot_suffix');
    }
    const v = await adb(['--version'], { quiet: true });
    info.adbVersion = (v.out.match(/Version [\d.]+-?\S*/) || ['adb not found'])[0];
    return info;
  },

  async pickBuild({ build }) {
    const t = await latest(build);
    const current = (await deviceState()) === 'device' ? await prop('ro.lineage.version') : '';
    return { filename: t.rel.filename, size: t.rel.size, current, majorChange: current && !current.startsWith(`${t.major}.`) };
  },

  async download() {
    const t = needTarget();
    fs.mkdirSync(t.dir, { recursive: true });
    fs.mkdirSync(path.dirname(t.zip), { recursive: true });
    await download(t.rel.url, t.zip, t.rel.filename);
    log('Checking SHA-256...', 'info');
    if ((await sha256(t.zip)) !== t.rel.id.toLowerCase()) throw new Error('SHA-256 mismatch. Delete the zip and retry.');
    log('SHA-256 OK', 'ok');
    for (const img of ['recovery.img', 'init_boot.img', 'build-manifest.xml']) {
      await download(`https://mirrorbits.lineageos.org/full/${settings.device}/${t.date}/${img}`, path.join(t.dir, img), img);
    }
    if (!sizeIs(path.join(t.dir, 'recovery.img'), RECOVERY_SIZE)) throw new Error('recovery.img size is wrong.');
    return 'Downloaded and verified.';
  },

  async extractFox() {
    const zip = abs(settings.foxZip);
    if (!fs.existsSync(zip)) throw new Error(`Missing ${zip}`);
    const dest = artifact('fox-extract');
    await must(run('powershell.exe', ['-NoProfile', '-Command', `Expand-Archive -LiteralPath '${zip}' -DestinationPath '${dest}' -Force`]));
    if (!sizeIs(path.join(dest, 'recovery.img'), RECOVERY_SIZE)) throw new Error('OrangeFox recovery.img is truncated. Do not flash it.');
    return 'OrangeFox recovery.img ready (104857600 bytes).';
  },

  async flashLineageRecovery() {
    const t = needTarget();
    await toBootloader();
    await must(fastboot(['flash', 'recovery', path.join(t.dir, 'recovery.img')]));
    await must(fastboot(['reboot', 'recovery']));
    return 'On the phone: Apply update -> Apply from ADB, then run the next step.';
  },

  async sideloadRom() {
    const t = needTarget();
    await waitFor('sideload');
    await adb(['sideload', t.zip]); // stopping near 47% with "Total xfer: 1.00x" is normal and can exit non-zero
    return 'Wait for the phone to say it finished. Then on the phone: Advanced -> Reboot to recovery, then Apply update -> Apply from ADB.';
  },

  async sideloadGapps() {
    const zip = abs(settings.gapps);
    if (!fs.existsSync(zip)) throw new Error(`Missing ${zip}`);
    await waitFor('sideload');
    await adb(['sideload', zip]);
    return 'Answer Yes to the signature warning. Then tap Reboot system now.';
  },

  async waitBoot() {
    await waitBoot();
    return `Booted: ${await prop('ro.lineage.version')}`;
  },

  async flashFox() {
    const img = artifact('fox-extract', 'recovery.img');
    if (!sizeIs(img, RECOVERY_SIZE)) throw new Error('Run "Extract OrangeFox" first.');
    await toBootloader();
    await must(fastboot(['flash', 'recovery', img]));
    await must(fastboot(['reboot']));
    await waitBoot();
    return 'OrangeFox flashed and the phone booted.';
  },

  async pushInitBoot() {
    const t = needTarget();
    await must(adb(['push', path.join(t.dir, 'init_boot.img'), `/sdcard/Download/init_boot_${t.date}.img`]));
    const kernel = (await adb(['shell', 'uname', '-r'], { quiet: true })).out.trim();
    return `Kernel ${kernel}. In KernelSU-Next: Install -> Select and patch a file -> init_boot_${t.date}.img, and set the KMI to ${settings.kmi} by hand.`;
  },

  async pullAndFlashKsu() {
    const t = needTarget();
    const ls = await must(adb(['shell', 'ls -t /sdcard/Download/kernelsu_next_patched_*.img'], { quiet: true }));
    const remote = ls.split(/\r?\n/)[0].trim();
    if (!remote) throw new Error('No kernelsu_next_patched_*.img in /sdcard/Download.');
    const out = artifact('images', `ksu-patched-init_boot-${t.date}.img`);
    fs.mkdirSync(path.dirname(out), { recursive: true });
    await must(adb(['pull', remote, out]));
    const ver = (fs.readFileSync(out).toString('latin1').match(/vermagic=[^\x00]{0,40}/) || ['vermagic not found'])[0];
    log(ver, 'info');
    const want = settings.kmi.split('-')[1]; // android15-6.6 -> 6.6
    if (!ver.startsWith(`vermagic=${want}.`)) throw new Error(`Wrong KMI in the patched image (${ver}). Re-patch with ${settings.kmi}.`);
    await toBootloader();
    await must(fastboot(['flash', 'init_boot', out]));
    await must(fastboot(['reboot']));
    await waitBoot();
    return 'Root flashed. Open the KernelSU-Next manager and confirm it says Working.';
  },

  async rollbackRoot() {
    const t = needTarget();
    await toBootloader();
    await must(fastboot(['flash', 'init_boot', path.join(t.dir, 'init_boot.img')]));
    await must(fastboot(['reboot']));
    return 'Stock init_boot restored.';
  },

  async reboot({ to }) {
    const state = await deviceState();
    if (state === 'fastboot') await must(fastboot(to === 'system' ? ['reboot'] : ['reboot', to]));
    else await must(adb(to === 'system' ? ['reboot'] : ['reboot', to]));
    return `Rebooting to ${to}.`;
  },

  // Free-form adb/fastboot command, with the wipe commands blocked.
  async custom({ line }) {
    const parts = line.match(/"[^"]*"|\S+/g)?.map((s) => s.replace(/^"|"$/g, '')) || [];
    const [cmd, ...args] = parts;
    if (!['adb', 'fastboot'].includes(cmd)) throw new Error('Only adb and fastboot commands are allowed here.');
    const joined = args.join(' ').toLowerCase();
    if (/(^|\s)-w(\s|$)|erase\s+userdata|format|wipe/.test(joined)) throw new Error('Wipe commands are blocked in the toolbox.');
    await run(tool(cmd), args);
    return 'Done.';
  },
};

async function toBootloader() {
  if ((await deviceState()) !== 'fastboot') {
    await must(adb(['reboot', 'bootloader']));
    await waitFor('fastboot', 60);
  }
}

// --- IPC ---
ipcMain.handle('step', async (_e, name, args = {}) => {
  if (name !== 'status') cancelled = false;
  try {
    return { ok: true, result: await steps[name](args) };
  } catch (e) {
    log(e.message, 'err');
    win?.webContents.send('progress', null);
    return { ok: false, error: e.message };
  }
});
ipcMain.handle('cancel', () => { cancelled = true; downloading?.abort(); running?.kill(); running = null; });
ipcMain.handle('settings:get', () => settings);
ipcMain.handle('settings:set', (_e, s) => {
  settings = { ...settings, ...s };
  fs.mkdirSync(path.dirname(settingsFile()), { recursive: true });
  fs.writeFileSync(settingsFile(), JSON.stringify(settings, null, 2));
  return settings;
});
ipcMain.handle('pick', async (_e, { folder, current }) => {
  const r = await dialog.showOpenDialog(win, {
    defaultPath: current ? abs(current) : settings.root,
    properties: [folder ? 'openDirectory' : 'openFile'],
  });
  return r.canceled ? null : r.filePaths[0];
});
ipcMain.handle('openRoot', () => shell.openPath(settings.root));
ipcMain.handle('runScript', () => {
  // Hand off to the original PowerShell script in its own console window.
  spawn('cmd.exe', ['/c', 'start', 'powershell', '-NoExit', '-ExecutionPolicy', 'Bypass', '-File', path.join(settings.root, 'scripts', 'update-lineage.ps1')], {
    cwd: settings.root, detached: true, stdio: 'ignore',
  }).unref();
});

app.whenReady().then(() => {
  settings = loadSettings();
  win = new BrowserWindow({
    width: 1100, height: 780, title: 'Dodge Toolbox',
    webPreferences: { preload: path.join(__dirname, 'preload.js'), contextIsolation: true },
  });
  win.setMenuBarVisibility(false);
  win.loadFile('index.html');
});
app.on('window-all-closed', () => app.quit());
