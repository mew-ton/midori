import path from "node:path";
import { fileURLToPath } from "node:url";
import { app, BrowserWindow } from "electron";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Dev mode loads the Astro dev server. The packaged-app path (which would
// spawn the Astro Node SSR bundle from `dist/server/entry.mjs`) is a
// follow-up Issue and is intentionally not wired here.
const DEV_URL = process.env.MIDORI_GUI_DEV_URL ?? "http://127.0.0.1:4321";

async function createWindow() {
  const win = new BrowserWindow({
    width: 1280,
    height: 800,
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  await win.loadURL(DEV_URL);
}

app.whenReady().then(createWindow);

app.on("window-all-closed", () => {
  // macOS keeps the app alive when all windows close (standard convention);
  // every other platform exits.
  if (process.platform !== "darwin") app.quit();
});

app.on("activate", () => {
  // macOS re-opens a window when the dock icon is clicked and no windows are
  // open.
  if (BrowserWindow.getAllWindows().length === 0) createWindow();
});

// Re-export __dirname for consumers that need the package root path; kept to
// avoid the "unused import" lint after stripping the dev/prod branch above.
export { __dirname };
