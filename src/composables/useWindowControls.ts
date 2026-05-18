import { getCurrentWindow } from "@tauri-apps/api/window";

export function useWindowControls() {
  const appWindow = getCurrentWindow();

  async function minimizeWindow() {
    await appWindow.minimize();
  }

  async function toggleMaximizeWindow() {
    await appWindow.toggleMaximize();
  }

  async function closeWindow() {
    await appWindow.close();
  }

  async function startWindowDrag(event: MouseEvent) {
    if (event.button !== 0) return;
    await appWindow.startDragging();
  }

  async function handleTitlebarDoubleClick(event: MouseEvent) {
    if (event.button !== 0) return;
    await appWindow.toggleMaximize();
  }

  return {
    minimizeWindow,
    toggleMaximizeWindow,
    closeWindow,
    startWindowDrag,
    handleTitlebarDoubleClick
  };
}
