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
  const normalizedText = (element) =>
    (element.textContent || "").replace(/\s+/g, " ").trim().toLowerCase();
  const visibleText = (
    copies,
    selectors = 'button,div,p,span,h1,h2,h3,[role="heading"]',
  ) => {
    const lowered = copies.map((copy) => copy.toLowerCase());
    for (const element of document.querySelectorAll(selectors)) {
      if (!visible(element)) continue;
      const text = normalizedText(element);
      const match = lowered.find((copy) => text.includes(copy));
      if (match) return match;
    }
    return null;
  };
  const visibleSelectorCount = (selectors) =>
    selectors.reduce((count, selector) => {
      try {
        return (
          count +
          Array.from(document.querySelectorAll(selector)).filter(visible).length
        );
      } catch {
        return count;
      }
    }, 0);
  const participantTileLabel = (element) =>
    (
      element.getAttribute("aria-label") || (element.textContent || "").trim()
    ).slice(0, 128);

  const explicitDenial = visibleText([
    "denied your request",
    "request to join was denied",
    "you were denied",
    "weren't allowed to join",
    "weren’t allowed to join",
    "not allowed to join",
    "not admitted",
    "ask to join again",
    "can't join this video call",
    "can’t join this video call",
    "cannot join this video call",
  ]);
  const ambiguousError = visibleText([
    "meeting not found",
    "can't join the meeting",
    "unable to join",
    "access denied",
    "meeting has ended",
    "this meeting has ended",
    "invalid meeting",
    "meeting link expired",
    "try again",
    "retry",
    "go back",
  ]);
  const waitingRoom = Boolean(
    visibleText([
      "asking to be let in",
      "you'll join the call when someone lets you in",
      "you’ll join the call when someone lets you in",
      "please wait until a meeting host brings you into the call",
      "waiting for the host to let you in",
      "you're in the waiting room",
    ]) ||
    visibleSelectorCount([
      '[aria-label*="waiting room" i]',
      '[aria-label*="asking to be let in" i]',
      '[aria-label*="waiting for admission" i]',
    ]) > 0,
  );
  const consentPrompt = Boolean(
    visibleText(
      ["take notes for me", "taking notes"],
      '[role="dialog"],[role="alertdialog"]',
    ),
  );
  const recaptchaChallenge = Array.from(
    document.querySelectorAll('iframe[src*="recaptcha"]'),
  ).some((element) => {
    if (!visible(element)) return false;
    const rect = element.getBoundingClientRect();
    return rect.width >= 120 && rect.height >= 40;
  });
  const participantTileLabels = Array.from(
    document.querySelectorAll("[data-participant-id]"),
  )
    .slice(0, 128)
    .map(participantTileLabel);

  return {
    waiting_room_visible: waitingRoom,
    consent_prompt_visible: consentPrompt,
    explicit_denial_indicator: explicitDenial,
    ambiguous_error_indicator: ambiguousError,
    visible_recaptcha_challenge: recaptchaChallenge,
    participant_tile_labels: participantTileLabels,
    self_name_nodes: document.querySelectorAll("[data-self-name]").length,
    visible_admission_controls: visibleSelectorCount([
      'button[aria-label*="share screen" i]',
      'button[aria-label*="present now" i]',
      'button[aria-label*="leave call" i]',
      'button[aria-label*="leave meeting" i]',
    ]),
  };
})();
