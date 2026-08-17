// Adapted for Anarlog from Vexa v0.12.18. See ../THIRD_PARTY_NOTICES.md.
(() => {
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
  const visibleText = (copies) => {
    const lowered = copies.map((copy) => copy.toLowerCase());
    for (const element of document.querySelectorAll(
      'button,div,p,span,[role="dialog"],[role="alertdialog"]',
    )) {
      if (!visible(element)) continue;
      const text = (element.textContent || "")
        .replace(/\s+/g, " ")
        .trim()
        .toLowerCase();
      const match = lowered.find((copy) => text.includes(copy));
      if (match) return match;
    }
    return null;
  };
  const visibleCount = (selector) =>
    Array.from(document.querySelectorAll(selector)).filter(visible).length;

  return {
    removal_indicator: visibleText([
      "you've been removed from this meeting",
      "you have been removed from this meeting",
      "you were removed from the meeting",
      "removed you from the meeting",
    ]),
    meeting_ended_indicator: visibleText([
      "meeting ended",
      "this meeting has ended",
      "call ended",
      "you left the meeting",
    ]),
    connection_problem_indicator: visibleText([
      "connection lost",
      "unable to connect",
      "reconnecting",
    ]),
    participant_tile_labels: Array.from(
      document.querySelectorAll("[data-participant-id]"),
    ).map(
      (element) =>
        element.getAttribute("aria-label") ||
        (element.textContent || "").trim(),
    ),
    self_name_nodes: document.querySelectorAll("[data-self-name]").length,
    visible_meeting_controls:
      visibleCount('button[aria-label*="leave call" i]') +
      visibleCount('button[aria-label*="leave meeting" i]') +
      visibleCount('button[aria-label*="people" i]') +
      visibleCount('button[aria-label*="chat" i]'),
  };
})();
