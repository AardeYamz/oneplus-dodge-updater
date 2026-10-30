const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('toolbox', {
  step: (name, args) => ipcRenderer.invoke('step', name, args),
  cancel: () => ipcRenderer.invoke('cancel'),
  getSettings: () => ipcRenderer.invoke('settings:get'),
  setSettings: (s) => ipcRenderer.invoke('settings:set', s),
  pick: (opts) => ipcRenderer.invoke('pick', opts),
  openRoot: () => ipcRenderer.invoke('openRoot'),
  runScript: () => ipcRenderer.invoke('runScript'),
  onLog: (fn) => ipcRenderer.on('log', (_e, m) => fn(m)),
  onProgress: (fn) => ipcRenderer.on('progress', (_e, p) => fn(p)),
});
