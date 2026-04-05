import { useEffect, useState } from "react";

const MAIN_SHELL_SELECTOR = "[data-testid='main-app-shell']";

export function useMainContentCenterOffset() {
  const [contentOffset, setContentOffset] = useState(0);

  useEffect(() => {
    const computeOffset = () => {
      const shell = document.querySelector(MAIN_SHELL_SELECTOR);
      if (!shell) {
        setContentOffset(0);
        return;
      }

      const panels = document.querySelectorAll("[data-panel-id]");
      const bodyPanel = panels[0];
      if (!bodyPanel) {
        setContentOffset(0);
        return;
      }

      const bodyRect = bodyPanel.getBoundingClientRect();
      const bodyCenter = bodyRect.left + bodyRect.width / 2;
      const windowCenter = window.innerWidth / 2;
      setContentOffset(bodyCenter - windowCenter);
    };

    computeOffset();
    window.addEventListener("resize", computeOffset);

    const resizeObserver = new ResizeObserver(computeOffset);
    const panels = document.querySelectorAll("[data-panel-id]");
    for (const panel of panels) {
      resizeObserver.observe(panel);
    }

    return () => {
      window.removeEventListener("resize", computeOffset);
      resizeObserver.disconnect();
    };
  }, []);

  return contentOffset;
}
