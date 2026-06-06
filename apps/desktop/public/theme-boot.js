(function () {
  // Fast path before React boots. `main.tsx` re-reads settings.json and syncs this key.
  var stored = localStorage.getItem("hypr-theme");
  var theme =
    stored === "light" || stored === "dark" || stored === "system"
      ? stored
      : "system";
  var prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  var isDark =
    theme === "dark" ? true : theme === "light" ? false : prefersDark;
  document.documentElement.classList.toggle("dark", isDark);
})();
