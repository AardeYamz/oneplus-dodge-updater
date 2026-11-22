// UI for the step list, log and settings. All real work happens in the Rust side (src/) via window.toolbox (bridge.js).
const $ = (id) => document.getElementById(id);
const tb = window.toolbox;

// Mirrors scripts/update-lineage.ps1 / the manual flash instructions in docs/reference.md. `tap` is shown before running; `confirm` asks first; `pause` stops the autorun for a prompt before the step.
const STEPS = [
  { id: 'pickBuild', title: 'Pick the build', desc: 'Asks the LineageOS API for the newest nightly (or the date you type).', input: 'Build date yyyyMMdd (blank = newest)' },
  { id: 'download', title: 'Download and verify', desc: 'ROM zip (SHA-256 checked), recovery.img, init_boot.img and build-manifest.xml. Skips files you already have.' },
  { id: 'extractFox', title: 'Extract OrangeFox', desc: 'Unzips the OrangeFox zip to artifacts\\fox-extract\\ and checks recovery.img is 104857600 bytes.' },
  { id: 'flashLineageRecovery', title: 'Flash LineageOS Recovery', desc: 'adb reboot bootloader → fastboot flash recovery → reboot to recovery.', confirm: 'Reboot to the bootloader and flash LineageOS Recovery?' },
  { id: 'sideloadRom', title: 'Sideload LineageOS', desc: 'Stopping near 47% with "Total xfer: 1.00x" is normal.', tap: 'On the phone: Apply update → Apply from ADB.' },
  { id: 'sideloadGapps', title: 'Switch slot, sideload MindTheGapps', desc: 'Do not reboot to system before this.', tap: 'On the phone: Advanced → Reboot to recovery, then Apply update → Apply from ADB. Make sure the slot switch really happened: a "sideload" state can be the old session.', confirm: 'Did the phone reboot to recovery (slot switch) and is it on Apply from ADB again?' },
  { id: 'waitBoot', title: 'First boot', desc: 'Waits for sys.boot_completed. The first boot can take several minutes.', tap: 'On the phone: Reboot system now.' },
  { id: 'flashFox', title: 'Flash OrangeFox (last)', desc: 'Replaces LineageOS Recovery with OrangeFox. Keeps data.', confirm: 'Reboot to the bootloader and flash OrangeFox recovery?' },
  { id: 'pushInitBoot', title: 'Push init_boot for KernelSU-Next', desc: 'Copies the new build\'s init_boot.img to /sdcard/Download.', pause: 'The phone is still doing its first boot after OrangeFox. Wait for it to finish, then unlock it: storage stays unavailable while it is locked and the push fails.' },
  { id: 'pullAndFlashKsu', title: 'Pull, check and flash the patched init_boot', desc: 'Checks vermagic matches the KMI before flashing.', tap: 'In KernelSU-Next: Install → Select and patch a file → the pushed init_boot. Set the KMI manually (android15-6.6); the auto pick is wrong.', confirm: 'Patched with the right KMI? Flash it to init_boot?' },
];

const SETTINGS = [
  ['dataRoot', 'Data folder (holds artifacts\\ and platform-tools\\; X: drive)', true],
  ['root', 'App folder (holds scripts\\)', true],
  ['platformTools', 'platform-tools folder (adb.exe / fastboot.exe)', true],
  ['gapps', 'MindTheGapps zip', false],
  ['foxZip', 'OrangeFox zip', false],
  ['device', 'Device codename', null],
  ['kmi', 'KernelSU KMI', null],
];

// Themed replacement for window.confirm(): resolves true on OK, false on Cancel/Esc.
function ask(title, body, { ok = 'OK', cancel = 'Cancel' } = {}) {
  return new Promise((resolve) => {
    const dlg = document.createElement('dialog');
    dlg.className = 'ask';
    const h = document.createElement('h3');
    h.textContent = title;
    const p = document.createElement('p');
    p.textContent = body;
    const row = document.createElement('div');
    row.className = 'row';
    const yes = document.createElement('button');
    yes.className = 'primary';
    yes.textContent = ok;
    const no = document.createElement('button');
    no.textContent = cancel;
    row.append(yes, no);
    dlg.append(h, p, row);
    yes.onclick = () => dlg.close('ok');
    no.onclick = () => dlg.close('cancel');
    dlg.onclose = () => { dlg.remove(); resolve(dlg.returnValue === 'ok'); };
    document.body.append(dlg);
    dlg.showModal();
    yes.focus();
  });
}

let busy = false;
const runners = [];

function log({ text, kind }) {
  const span = document.createElement('span');
  span.className = kind;
  span.textContent = text.endsWith('\n') ? text : text + '\n';
  $('log').append(span);
  $('log').scrollTop = $('log').scrollHeight;
}

function setBusy(on) {
  busy = on;
  document.querySelectorAll('button:not(#cancel):not(#clear):not([data-yl-theme-toggle])').forEach((b) => (b.disabled = on));
  $('cancel').disabled = !on;
}

async function call(name, args, li) {
  if (busy) return null;
  setBusy(true);
  li?.classList.remove('done', 'failed');
  li?.classList.add('active');
  const r = await tb.step(name, args);
  li?.classList.remove('active');
  li?.classList.add(r.ok ? 'done' : 'failed');
  setBusy(false);
  refresh();
  return r;
}

function renderSteps() {
  for (const s of STEPS) {
    const li = document.createElement('li');
    li.className = 'step';
    li.innerHTML = `<h3><span></span><button class="primary">Run</button></h3><p></p>`;
    li.querySelector('span').textContent = s.title;
    li.querySelector('p').textContent = s.desc;
    if (s.tap) {
      const tap = document.createElement('div');
      tap.className = 'tap';
      tap.textContent = '📱 ' + s.tap;
      li.append(tap);
    }
    let input;
    if (s.input) {
      input = document.createElement('input');
      input.placeholder = s.input;
      const row = document.createElement('div');
      row.className = 'row';
      row.append(input);
      li.append(row);
    }
    const result = document.createElement('div');
    result.className = 'result';
    li.append(result);

    const run = async (auto) => {
      if (!auto && s.confirm && !(await ask(s.title, s.confirm))) return false;
      const r = await call(s.id, input ? { build: input.value.trim() } : {}, li);
      if (!r) return false;
      if (!r.ok) { result.textContent = '✖ ' + r.error; return false; }
      result.textContent = typeof r.result === 'string' ? r.result : describeBuild(r.result);
      if (s.id === 'pickBuild') log({ text: result.textContent, kind: r.result.majorChange ? 'err' : 'ok' });
      return true;
    };
    li.querySelector('button').onclick = () => run(false);
    runners.push({ s, li, run });
    $('steps').append(li);
  }
}

// Chains the steps from the chosen one. Phone taps and the reboot/flash prompts are asked once per step; Cancel in a prompt, a failed step or the Cancel button stops the run.
async function autorun() {
  if (busy) return;
  const from = Number($('from').value);
  const todo = runners.slice(from);
  const flashes = todo.filter((r) => r.s.confirm).map((r) => '• ' + r.s.title).join('\n');
  if (!(await ask('Autorun', `Run ${todo.length} steps from "${todo[0].s.title}" to the end?\n\nThis reboots the phone and flashes:\n${flashes}\n\nYou'll be asked to confirm the phone taps before each step that needs them.`, { ok: 'Start' }))) return;
  log({ text: '--- Autorun started ---', kind: 'info' });
  for (const { s, li, run } of todo) {
    if (s.tap && !(await ask(s.title, `📱 ${s.tap}\n\nOK when the phone is ready, Cancel to stop the autorun.`))) {
      log({ text: `Autorun stopped before "${s.title}".`, kind: 'err' });
      return;
    }
    if (s.pause && !(await ask(s.title, `⏸ ${s.pause}\n\nOK to continue, Cancel to stop the autorun.`))) {
      log({ text: `Autorun stopped before "${s.title}".`, kind: 'err' });
      return;
    }
    if (!(await run(true))) {
      log({ text: `Autorun stopped at "${s.title}".`, kind: 'err' });
      return;
    }
  }
  log({ text: '--- Autorun finished ---', kind: 'ok' });
}

function describeBuild(b) {
  const gb = (b.size / 1e9).toFixed(2);
  let text = `Target ${b.filename} (${gb} GB). Installed: ${b.current || 'phone not in adb'}.`;
  if (b.current && b.current.includes(b.filename.match(/lineage-([\d.]+-\d{8})/)[1])) text += ' Already on this build.';
  if (b.majorChange) text += ' ⚠ Major version change: check the wiki for a required wipe and back up first.';
  return text;
}

async function refresh() {
  if (busy) return;
  const r = await tb.step('status');
  if (!r.ok) return;
  const s = r.result;
  $('state').textContent = s.state;
  $('state').className = 'pill ' + s.state;
  const bits = [s.version, s.kernel && 'kernel ' + s.kernel, s.slot && 'slot ' + s.slot, s.adbVersion].filter(Boolean);
  $('info').textContent = bits.join(' · ');
  if (/1\.0\.39/.test(s.adbVersion)) $('info').textContent += ' ⚠ old Essential ADB: set the platform-tools folder in Settings';
}

async function renderSettings() {
  const s = await tb.getSettings();
  const box = $('settings');
  box.innerHTML = '';
  for (const [key, label, folder] of SETTINGS) {
    const l = document.createElement('label');
    l.textContent = label;
    const row = document.createElement('div');
    row.className = 'row';
    const input = document.createElement('input');
    input.value = s[key];
    input.onchange = () => tb.setSettings({ [key]: input.value.trim() });
    row.append(input);
    if (folder !== null) {
      const b = document.createElement('button');
      b.textContent = 'Browse…';
      b.onclick = async () => {
        const p = await tb.pick({ folder, current: input.value });
        if (p) { input.value = p; await tb.setSettings({ [key]: p }); refresh(); }
      };
      row.append(b);
    }
    box.append(l, row);
  }
}

tb.onLog(log);
tb.onProgress((p) => {
  $('progress').hidden = !p;
  if (!p) return;
  const pct = p.total ? (p.done / p.total) * 100 : 0;
  $('fill').style.width = pct + '%';
  $('ptext').textContent = `${p.label}  ${(p.done / 1e6).toFixed(0)} / ${p.total ? (p.total / 1e6).toFixed(0) : '?'} MB`;
});

$('refresh').onclick = refresh;
$('autorun').onclick = autorun;
$('latest').onclick = async () => {
  const r = await call('pickBuild', { build: '' });
  if (r?.ok) log({ text: describeBuild(r.result), kind: r.result.majorChange ? 'err' : 'ok' });
};
$('cancel').onclick = () => tb.cancel();
$('clear').onclick = () => ($('log').textContent = '');
$('openRoot').onclick = () => tb.openRoot();
$('runScript').onclick = () => tb.runScript();
document.querySelectorAll('[data-reboot]').forEach((b) => {
  b.onclick = async () => (await ask('Reboot', `Reboot to ${b.dataset.reboot}?`)) && call('reboot', { to: b.dataset.reboot });
});
$('custom').onsubmit = (e) => {
  e.preventDefault();
  const line = $('line').value.trim();
  if (line) call('custom', { line });
};

renderSteps();
STEPS.forEach((s, i) => $('from').append(new Option(`${i + 1}. ${s.title}`, i)));
renderSettings();
refresh();
