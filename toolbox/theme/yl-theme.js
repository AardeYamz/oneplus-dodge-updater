/* yl-theme.js: the color-mode toggle of v3.yannislam.org (src/app/services/theme/theme.service.ts), for the project pages
   (trips, 37-bestick-rd, time-location-visual). Source: the yl-theme repo (src/yl-theme.js); edit it there and run
   scripts/build.py and scripts/sync.py, which overwrite the copies in the project repos.

   Modes cycle default -> light -> dark (the button's icon is half-circle / sun / moon, as on v3). Default sets no
   [data-theme]; the others set data-theme="light" / "dark" on <html>. The choice is stored under the same key as v3
   ("yl-theme-mode"); until one is stored the page follows the OS: light when it prefers light, else default.
   Load it in <head>, before the page renders, so the first paint is already in the right mode.

   Markup: <button type="button" class="yl-theme-toggle" data-yl-theme-toggle></button> and
   <a class="yl-brand" href="https://yannislam.org" aria-label="Yannis Lam, home" data-yl-logo></a>; the icons and
   the logo are filled in here (and v3's bars/xmark icons in a <button class="yl-menu-toggle" data-yl-menu="ID">, which
   opens and closes #ID), and a sticky header (.yl-head.sticky) gets --yl-head-h and .yl-stuck. A "yl-themechange" event ({detail:{mode}}) fires on window after every change. */
(function () {
  var KEY = 'yl-theme-mode';
  var MODES = ['default', 'light', 'dark'];
  // --color-navy per mode (yl-theme.css): keeps the mobile browser chrome in step with the page.
  var NAVY = { default: '#131f31', light: '#f4f1ea', dark: '#05070c' };
  var ICONS = {
    'default': ['0 0 512 512', 'M448 256c0-106-86-192-192-192l0 384c106 0 192-86 192-192zM0 256a256 256 0 1 1 512 0 256 256 0 1 1 -512 0z'],
    light: ['0 0 576 512', 'M288-32c8.4 0 16.3 4.4 20.6 11.7L364.1 72.3 468.9 46c8.2-2 16.9 .4 22.8 6.3S500 67 498 75.1l-26.3 104.7 92.7 55.5c7.2 4.3 11.7 12.2 11.7 20.6s-4.4 16.3-11.7 20.6L471.7 332.1 498 436.8c2 8.2-.4 16.9-6.3 22.8S477 468 468.9 466l-104.7-26.3-55.5 92.7c-4.3 7.2-12.2 11.7-20.6 11.7s-16.3-4.4-20.6-11.7L211.9 439.7 107.2 466c-8.2 2-16.8-.4-22.8-6.3S76 445 78 436.8l26.2-104.7-92.6-55.5C4.4 272.2 0 264.4 0 256s4.4-16.3 11.7-20.6L104.3 179.9 78 75.1c-2-8.2 .3-16.8 6.3-22.8S99 44 107.2 46l104.7 26.2 55.5-92.6 1.8-2.6c4.5-5.7 11.4-9.1 18.8-9.1zm0 144a144 144 0 1 0 0 288 144 144 0 1 0 0-288zm0 240a96 96 0 1 1 0-192 96 96 0 1 1 0 192z'],
    open: ['0 0 448 512', 'M0 96C0 78.3 14.3 64 32 64l384 0c17.7 0 32 14.3 32 32s-14.3 32-32 32L32 128C14.3 128 0 113.7 0 96zM0 256c0-17.7 14.3-32 32-32l384 0c17.7 0 32 14.3 32 32s-14.3 32-32 32L32 288c-17.7 0-32-14.3-32-32zM448 416c0 17.7-14.3 32-32 32L32 448c-17.7 0-32-14.3-32-32s14.3-32 32-32l384 0c17.7 0 32 14.3 32 32z'],
    close: ['0 0 384 512', 'M55.1 73.4c-12.5-12.5-32.8-12.5-45.3 0s-12.5 32.8 0 45.3L147.2 256 9.9 393.4c-12.5 12.5-12.5 32.8 0 45.3s32.8 12.5 45.3 0L192.5 301.3 329.9 438.6c12.5 12.5 32.8 12.5 45.3 0s12.5-32.8 0-45.3L237.8 256 375.1 118.6c12.5-12.5 12.5-32.8 0-45.3s-32.8-12.5-45.3 0L192.5 210.7 55.1 73.4z'],
    dark: ['0 0 512 512', 'M256 0C114.6 0 0 114.6 0 256S114.6 512 256 512c68.8 0 131.3-27.2 177.3-71.4 7.3-7 9.4-17.9 5.3-27.1s-13.7-14.9-23.8-14.1c-4.9 .4-9.8 .6-14.8 .6-101.6 0-184-82.4-184-184 0-72.1 41.5-134.6 102.1-164.8 9.1-4.5 14.3-14.3 13.1-24.4S322.6 8.5 312.7 6.3C294.4 2.2 275.4 0 256 0z']
  };
  // v3 src/assets/images/logos/clearcolor.svg; yl-theme.css turns it black (light) or white (dark).
  var LOGO = [['#fff200', '287,194 287,80 343,193'], ['#fff200', '437,194 493,80 493,194'], ['#7f7f7f', '360,204 420,204 390,265'],
    ['#ffc90e', '354,333 289,203 348,203 384,275'], ['#ffc90e', '425,333 396,275 432,203 491,203'], ['#ed1c24', '361,342 390,284 419,342 390,402'],
    ['#b5e61d', '323,539 384,417 384,667 323,669'], ['#ffc90e', '395,412 458,538 458,670 396,670'], ['#22b14c', '321,680 377,680 352,731 295,731'],
    ['#ff7f27', '484,731 427,731 402,681 458,680'], ['#efe4b0', '601,731 496,731 471,681 575,680']];

  var root = document.documentElement;
  var prefersLight = window.matchMedia ? window.matchMedia('(prefers-color-scheme: light)') : null;

  function stored() {
    try { var v = localStorage.getItem(KEY); return MODES.indexOf(v) >= 0 ? v : null; } catch (e) { return null; }
  }
  function system() { return prefersLight && prefersLight.matches ? 'light' : 'default'; }
  function current() {
    var t = root.getAttribute('data-theme');
    return t === 'light' || t === 'dark' ? t : 'default';
  }
  function label() {
    var bs = document.querySelectorAll('[data-yl-theme-toggle]');
    for (var i = 0; i < bs.length; i++) {
      bs[i].title = 'Toggle color theme';
      bs[i].setAttribute('aria-label', 'Toggle color theme (currently ' + current() + ')');
    }
  }
  function apply(mode) {
    if (mode === 'default') root.removeAttribute('data-theme');
    else root.setAttribute('data-theme', mode);
    var meta = document.querySelector('meta[name="theme-color"]');
    if (!meta && document.head) {
      meta = document.createElement('meta');
      meta.name = 'theme-color';
      document.head.appendChild(meta);
    }
    if (meta) meta.setAttribute('content', NAVY[mode]);
    label();
    try { window.dispatchEvent(new CustomEvent('yl-themechange', { detail: { mode: mode } })); } catch (e) { }
  }
  function set(mode) {
    if (MODES.indexOf(mode) < 0) return;
    try { localStorage.setItem(KEY, mode); } catch (e) { }
    apply(mode);
  }
  function svg(viewBox, inner, cls) {
    return '<svg' + (cls ? ' class="' + cls + '"' : '') + ' viewBox="' + viewBox + '" aria-hidden="true" focusable="false">' + inner + '</svg>';
  }
  function fill() {
    var bs = document.querySelectorAll('[data-yl-theme-toggle]');
    for (var i = 0; i < bs.length; i++) {
      if (bs[i].firstElementChild) continue;
      bs[i].innerHTML = MODES.map(function (m) { return svg(ICONS[m][0], '<path d="' + ICONS[m][1] + '"/>', 'yl-ic-' + m); }).join('');
    }
    // menu buttons: bars / xmark (v3's icons)
    var ms = document.querySelectorAll('[data-yl-menu]');
    for (var m = 0; m < ms.length; m++) {
      if (ms[m].firstElementChild) continue;
      ms[m].innerHTML = ['open', 'close'].map(function (k) { return svg(ICONS[k][0], '<path d="' + ICONS[k][1] + '"/>', 'yl-ic-' + k); }).join('');
    }
    // version pills: wrap the time of "vYYYY-MM-DD HH:MM" so phones can show the date only
    var vs = document.querySelectorAll('.yl-ver');
    for (var v = 0; v < vs.length; v++) {
      var mt = /^(v\d{4}-\d{2}-\d{2})( \d{1,2}:\d{2})$/.exec(vs[v].textContent.trim());
      if (mt && !vs[v].firstElementChild) vs[v].innerHTML = mt[1] + '<span class="yl-ver-time">' + mt[2] + '</span>';
    }
    var ls = document.querySelectorAll('[data-yl-logo]');
    for (var j = 0; j < ls.length; j++) {
      if (ls[j].firstElementChild) continue;
      ls[j].innerHTML = svg('280 74 328 664', LOGO.map(function (p) { return '<polygon fill="' + p[0] + '" points="' + p[1] + '"/>'; }).join(''));
    }
    label();
    stickyHeader();
  }
  // Sticky header (.yl-head.sticky): publish its height as --yl-head-h for the bars that stick under it, and
  // mark it .yl-stuck once the page has scrolled (it then shows its bottom rule).
  function stickyHeader() {
    var h = document.querySelector('.yl-head.sticky');
    if (!h) return;
    var set = function () { root.style.setProperty('--yl-head-h', h.offsetHeight + 'px'); };
    set();
    if (window.ResizeObserver) new ResizeObserver(set).observe(h);
    else window.addEventListener('resize', set);
    var stuck = function () { h.classList.toggle('yl-stuck', window.scrollY > 0); };
    stuck();
    window.addEventListener('scroll', stuck, { passive: true });
  }

  apply(stored() || system());
  // Follow the OS live until a choice is stored.
  if (prefersLight && prefersLight.addEventListener) {
    prefersLight.addEventListener('change', function () { if (!stored()) apply(system()); });
  }
  document.addEventListener('click', function (e) {
    var b = e.target && e.target.closest ? e.target.closest('[data-yl-theme-toggle]') : null;
    if (b) set(MODES[(MODES.indexOf(current()) + 1) % MODES.length]);
  });

  // Menus: <button class="yl-menu-toggle" data-yl-menu="ID" aria-controls="ID"> toggles .open on #ID (and
  // aria-expanded). A click outside, a click on a link inside, or Escape closes it.
  function menu(btn, open) {
    var t = document.getElementById(btn.getAttribute('data-yl-menu'));
    if (!t) return;
    t.classList.toggle('open', open);
    btn.setAttribute('aria-expanded', open ? 'true' : 'false');
  }
  function closeMenus(except) {
    var bs = document.querySelectorAll('[data-yl-menu][aria-expanded="true"]');
    for (var i = 0; i < bs.length; i++) if (bs[i] !== except) menu(bs[i], false);
  }
  document.addEventListener('click', function (e) {
    var b = e.target && e.target.closest ? e.target.closest('[data-yl-menu]') : null;
    if (b) { closeMenus(b); menu(b, b.getAttribute('aria-expanded') !== 'true'); return; }
    var open = document.querySelectorAll('[data-yl-menu][aria-expanded="true"]');
    for (var i = 0; i < open.length; i++) {
      var t = document.getElementById(open[i].getAttribute('data-yl-menu'));
      if (!t || !t.contains(e.target) || (e.target.closest && e.target.closest('a'))) menu(open[i], false);
    }
  });
  document.addEventListener('keydown', function (e) {
    if (e.key !== 'Escape') return;
    var open = document.querySelector('[data-yl-menu][aria-expanded="true"]');
    if (open) { closeMenus(); open.focus(); }
  });
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', fill);
  else fill();

  window.ylTheme = { mode: current, set: set, isLight: function () { return current() === 'light'; } };
})();
