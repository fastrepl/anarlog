(function () {
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
