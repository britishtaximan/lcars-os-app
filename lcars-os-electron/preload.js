// LCARS OS — Electron Preload Script
// Exposes IPC as window.__TAURI_INTERNALS__.invoke() so the same
// index.html works in both Tauri and Electron without changes.

const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('__TAURI_INTERNALS__', {
  invoke: function(command, args) {
    return ipcRenderer.invoke(command, args);
  }
});
