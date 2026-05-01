import { callCommand } from "./commands.js";

export async function minimizeWindow() {
  const response = await callCommand("window_minimize");
  if (response.ok) return true;

  if (typeof window !== "undefined" && window.__TAURI__?.window?.getCurrentWindow) {
    await window.__TAURI__.window.getCurrentWindow().minimize();
    return true;
  }

  throw new Error(String(response.data?.error ?? "window_minimize failed"));
}

export async function toggleMaximizeWindow() {
  const response = await callCommand("window_toggle_maximize");
  if (response.ok) return true;

  if (typeof window !== "undefined" && window.__TAURI__?.window?.getCurrentWindow) {
    const current = window.__TAURI__.window.getCurrentWindow();
    const maximized = await current.isMaximized();
    if (maximized) await current.unmaximize();
    else await current.maximize();
    return true;
  }

  throw new Error(String(response.data?.error ?? "window_toggle_maximize failed"));
}

export async function closeWindow() {
  const response = await callCommand("window_close");
  if (response.ok) return true;

  if (typeof window !== "undefined" && window.__TAURI__?.window?.getCurrentWindow) {
    await window.__TAURI__.window.getCurrentWindow().close();
    return true;
  }

  throw new Error(String(response.data?.error ?? "window_close failed"));
}
