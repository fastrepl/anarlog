(function installAnarlogMeetingCamera(global) {
  "use strict";
  if (global.anarlogMeetingCamera) return;

  /**
   * Every Google Meet selector this uses, in one table, so a later build can take them from
   * remote config when Meet changes its page. Labels are English only for now.
   */
  const MEET = {
    leave: 'button[aria-label="Leave call"]',
    moreOptions: 'button[aria-label="More options"]',
    menuItem: '[role="menuitem"]',
    dialog: '[role="dialog"]',
    tab: '[role="tab"]',
    /** The camera picker in Settings > Video: a combobox or a button that opens a list. */
    picker:
      '[role="combobox"][aria-label^="Camera" i], [aria-haspopup="listbox"][aria-label^="Camera" i], [role="button"][aria-label^="Camera" i]',
    option: '[role="option"], [role="menuitemradio"]',
    close: 'button[aria-label="Close dialog"], button[aria-label="Close"]',
  };

  /** How long the person may stay in the green room before Anarlog gives up. */
  const JOIN_LIMIT_MS = 10 * 60_000;
  /** Time for the call to settle after joining, before Anarlog opens Settings. */
  const SETTLE_MS = 2_000;
  const STEP_MS = 5_000;
  const triedKey = () => `anarlog-meeting-camera:${global.location.pathname}`;

  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  async function waitFor(find, limit, every = 200) {
    const until = Date.now() + limit;
    for (;;) {
      const found = find();
      if (found) return found;
      if (Date.now() >= until) return null;
      await sleep(every);
    }
  }
  const visible = (element) => element && !element.closest("[aria-hidden='true'],[hidden]");
  const textOf = (element) => (element.textContent || "").replace(/\s+/g, " ").trim();
  const named = (selector, name, root = document) =>
    [...root.querySelectorAll(selector)].find(
      (element) =>
        visible(element) &&
        (textOf(element) === name || (element.getAttribute("aria-label") || "").trim() === name),
    ) || null;
  const dialog = () => [...document.querySelectorAll(MEET.dialog)].find(visible) || null;
  const picker = () => {
    const open = dialog();
    const found = open && open.querySelector(MEET.picker);
    return visible(found) ? found : null;
  };
  /** The camera the picker shows now: its label after "Camera", or its text. */
  const shownCamera = (element) =>
    (element.getAttribute("aria-label") || "").replace(/^camera\s*:?\s*/i, "").trim() ||
    textOf(element);
  const isChosen = (option) =>
    option.getAttribute("aria-selected") === "true" ||
    option.getAttribute("aria-checked") === "true";

  async function openSettings() {
    const more = document.querySelector(MEET.moreOptions);
    if (!visible(more)) return false;
    more.click();
    const item = await waitFor(() => named(MEET.menuItem, "Settings"), STEP_MS);
    if (!item) return false;
    item.click();
    if (!(await waitFor(dialog, STEP_MS))) return false;
    const video = await waitFor(() => named(MEET.tab, "Video", dialog() || document), STEP_MS);
    if (video) video.click();
    return true;
  }

  function closeSettings() {
    const open = dialog();
    const close = open && open.querySelector(MEET.close);
    if (close) close.click();
    else
      document.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", code: "Escape", keyCode: 27, bubbles: true }),
      );
  }

  /** `live` turns false once the recording stops or a newer select starts. */
  async function choose(name, live) {
    const control = await waitFor(picker, STEP_MS);
    if (!control) return "picker_unavailable";
    if (shownCamera(control) === name) return "already_using";
    control.click();
    const option = await waitFor(() => named(MEET.option, name), STEP_MS);
    if (!option) {
      // Close the list again, leaving the camera as the person had it.
      control.click();
      return "camera_missing";
    }
    if (isChosen(option)) return "already_using";
    if (!live()) {
      control.click();
      return "cancelled";
    }
    option.click();
    const took = await waitFor(() => {
      const now = picker();
      return now && shownCamera(now) === name ? true : null;
    }, STEP_MS);
    return took ? "switched" : "unconfirmed";
  }

  let current = 0;
  /** Stops a select still waiting to act, for a recording that has stopped. */
  function cancel() {
    current += 1;
  }

  /**
   * Picks the camera named `name` in Meet's own Settings, once the person is in the call, and at
   * most once per call: a later recording in the same call leaves the person's choice alone. It
   * only changes which camera Meet uses; a camera that is off stays off. It leaves Meet's own
   * visual effects alone: no recorded Meet page shows their controls to match, so Anarlog asks
   * the person to check them once this switches. Resolves to `switched`,
   * `already_using`, `already_tried`, `camera_missing`, `picker_unavailable`, `unconfirmed`,
   * `not_in_call`, or `cancelled`.
   */
  async function select(name) {
    if (typeof name !== "string" || !name.trim()) return "unconfirmed";
    const run = ++current;
    const live = () => run === current;
    if (global.sessionStorage.getItem(triedKey())) {
      if (document.querySelector(MEET.leave)) return "already_tried";
      // The mark belongs to a call that has ended; a later call on the same link gets its own try.
      global.sessionStorage.removeItem(triedKey());
    }
    if (!(await waitFor(() => !live() || document.querySelector(MEET.leave), JOIN_LIMIT_MS, 1_000)))
      return "not_in_call";
    await sleep(SETTLE_MS);
    if (!live()) return "cancelled";
    global.sessionStorage.setItem(triedKey(), String(Date.now()));
    if (!(await openSettings())) {
      // A menu left open by a failed attempt goes away too.
      if (dialog()) closeSettings();
      return "picker_unavailable";
    }
    try {
      return await choose(name.trim(), live);
    } finally {
      closeSettings();
    }
  }

  global.anarlogMeetingCamera = { MEET, select, cancel, timing: { SETTLE_MS, STEP_MS } };
})(globalThis);
