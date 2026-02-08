const { contextBridge, ipcRenderer, webUtils } = require("electron");

const EVENT_CHANNEL = "tiny-vid:event";

const bridge = {
  invoke(request) {
    return ipcRenderer.invoke("tiny-vid:invoke", request);
  },
  on(event, handler) {
    const listener = (_ipcEvent, message) => {
      if (!message || message.event !== event) {
        return;
      }
      handler(message.payload);
    };
    ipcRenderer.on(EVENT_CHANNEL, listener);
    return () => {
      ipcRenderer.removeListener(EVENT_CHANNEL, listener);
    };
  },
  openDialog(options) {
    return ipcRenderer.invoke("tiny-vid:dialog:open", options);
  },
  saveDialog(options) {
    return ipcRenderer.invoke("tiny-vid:dialog:save", options);
  },
  platform() {
    return ipcRenderer.invoke("tiny-vid:platform");
  },
  toMediaSrc(path) {
    return ipcRenderer.invoke("tiny-vid:to-media-src", path);
  },
  pathForFile(file) {
    return webUtils.getPathForFile(file);
  },
};

contextBridge.exposeInMainWorld("__TINY_VID_ELECTRON__", bridge);
