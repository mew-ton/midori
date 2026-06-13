import { app, BrowserWindow } from "electron";

// Dev mode loads the Astro dev server directly. Production loading of the
// built SSR bundle (`dist/server/entry.mjs`) is deferred and intentionally
// not wired here.
const DEV_URL = process.env.MIDORI_GUI_DEV_URL ?? "http://127.0.0.1:4321";

async function createWindow() {
  const win = new BrowserWindow({
    width: 1280,
    height: 800,
    webPreferences: {
      // Three-flag lockdown matching the security design contract: the
      // renderer must not have direct Node access, must run in its own
      // context, and must run inside the OS sandbox.
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
  await win.loadURL(DEV_URL);
}

// Each createWindow() awaits loadURL, which can reject if the Astro dev
// server is not yet reachable. Catch on both entry points so the rejection
// surfaces as a logged error instead of an unhandled promise rejection.
app.whenReady().then(() => {
  createWindow().catch((error) => {
    console.error("createWindow failed at app ready:", error);
  });
});

app.on("window-all-closed", () => {
  // macOS keeps the app alive when all windows close (standard convention);
  // every other platform exits.
  if (process.platform !== "darwin") app.quit();
});

app.on("activate", () => {
  // macOS re-opens a window when the dock icon is clicked and no windows are
  // open.
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow().catch((error) => {
      console.error("createWindow failed on activate:", error);
    });
  }
});
