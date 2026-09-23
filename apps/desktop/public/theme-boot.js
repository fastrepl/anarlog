(function () {
  // Fast path before React boots. `main.tsx` re-reads settings.json and syncs this key.
  var stored =
    localStorage.getItem("anarlog-theme") ??
    localStorage.getItem("hypr-theme");
  var theme =
    stored === "light" || stored === "dark" || stored === "system"
      ? stored
      : "system";
  var prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  var isDark =
    theme === "dark" ? true : theme === "light" ? false : prefersDark;
  document.documentElement.classList.toggle("dark", isDark);

  // The undecorated Linux main window is transparent; clip the boot splash
  // and app root to the same radius the shell uses once React mounts.
  var ua = navigator.userAgent;
  var isLinuxDesktop = /\bLinux\b/.test(ua) && !/Android/.test(ua);
  var internals = window.__TAURI_INTERNALS__;
  var label =
    internals &&
    internals.metadata &&
    internals.metadata.currentWindow &&
    internals.metadata.currentWindow.label;
  if (isLinuxDesktop && label === "main") {
    document.documentElement.dataset.roundedWindow = "";
  }
})();
