// Replaces Electron's preload.js: the same window.toolbox API, on top of Tauri's IPC. All real work happens in src/.
(() => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  window.toolbox = {
    step: (name, args) => invoke('step', { name, args: args || {} }),
    cancel: () => invoke('cancel'),
    getSettings: () => invoke('settings_get'),
    setSettings: (s) => invoke('settings_set', { s }),
    pick: ({ folder, current }) => invoke('pick', { folder: !!folder, current: current || null }),
    openRoot: () => invoke('open_root'),
    runScript: () => invoke('run_script'),
    onLog: (fn) => listen('log', (e) => fn(e.payload)),
    onProgress: (fn) => listen('progress', (e) => fn(e.payload)),
  };
})();
