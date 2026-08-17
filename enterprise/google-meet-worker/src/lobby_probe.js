// Adapted for Anarlog from Vexa v0.12.18. See ../THIRD_PARTY_NOTICES.md.
(() => {
  const marker = "data-anlg-worker-target";
  const visible = (element) => {
    if (!(element instanceof HTMLElement)) return false;
    const style = window.getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return (
      style.display !== "none" &&
      style.visibility !== "hidden" &&
      Number(style.opacity) !== 0 &&
      rect.width > 0 &&
      rect.height > 0
    );
  };
  const firstVisible = (selectors) => {
    for (const selector of selectors) {
      for (const element of document.querySelectorAll(selector)) {
        if (visible(element)) return element;
      }
    }
    return null;
  };
  const buttonText = (button) =>
    (button.textContent || "").replace(/\s+/g, " ").trim();
  const buttons = Array.from(document.querySelectorAll("button")).filter(
    (button) =>
      visible(button) &&
      !button.disabled &&
      button.getAttribute("aria-disabled") !== "true",
  );
  const exactCtaLabels = ["Ask to join", "Join now", "Switch here", "Join"];
  let joinCta = buttons.find((button) =>
    exactCtaLabels.includes(buttonText(button)),
  );
  if (!joinCta) {
    const iconSelector =
      'i,svg,img,[class*="material-icons"],[class*="material-symbols"],[data-icon-name]';
    const structural = buttons.filter((button) => {
      if (
        button.hasAttribute("aria-label") ||
        button.querySelector(iconSelector)
      ) {
        return false;
      }
      const text = buttonText(button);
      return (
        text.length > 0 &&
        text.length <= 48 &&
        !text.includes("_") &&
        /\p{L}/u.test(text)
      );
    });
    if (structural.length === 1) joinCta = structural[0];
  }

  const nameInput = firstVisible([
    'input[jsname][type="text"]',
    'div[jscontroller] input[type="text"]',
    'input[type="text"][aria-label="Your name"]',
    'input[type="text"]:not([aria-hidden="true"])',
  ]);
  const microphone = firstVisible([
    'button[aria-label*="Turn off microphone" i]',
  ]);
  const camera = firstVisible(['button[aria-label*="Turn off camera" i]']);
  const deviceErrorCopies = [
    "mic not found",
    "speaker not found",
    "camera not found",
  ];
  const deviceErrorDismissal = buttons.find((button) => {
    if (
      !["close", "dismiss", "got it"].includes(buttonText(button).toLowerCase())
    ) {
      return false;
    }
    let ancestor = button.parentElement;
    for (let depth = 0; ancestor && depth < 8; depth += 1) {
      const text = buttonText(ancestor).toLowerCase();
      if (deviceErrorCopies.some((copy) => text.includes(copy))) return true;
      ancestor = ancestor.parentElement;
    }
    return false;
  });

  for (const element of document.querySelectorAll(`[${marker}]`)) {
    element.removeAttribute(marker);
  }
  const target = (element, kind) => {
    if (!element) return null;
    element.setAttribute(marker, kind);
    const rect = element.getBoundingClientRect();
    return {
      center_x: rect.left + rect.width / 2,
      center_y: rect.top + rect.height / 2,
    };
  };

  return {
    screen_x: window.screenX,
    screen_y: window.screenY,
    inner_width: window.innerWidth,
    inner_height: window.innerHeight,
    device_pixel_ratio: window.devicePixelRatio || 1,
    signed_out_lobby: Boolean(nameInput),
    name_input: target(nameInput, "name_input"),
    join_cta: target(joinCta, "join_cta"),
    microphone_on: target(microphone, "microphone_on"),
    camera_on: target(camera, "camera_on"),
    device_error_dismissal: target(
      deviceErrorDismissal,
      "device_error_dismissal",
    ),
    cta_candidates: buttons
      .slice(0, 64)
      .map((button) => buttonText(button).slice(0, 64))
      .filter(Boolean),
  };
})();
