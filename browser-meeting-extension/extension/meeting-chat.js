(function installAnarlogMeetingChat(global) {
  "use strict";
  if (global.anarlogMeetingChat) return;

  /**
   * Every Google Meet selector this uses, in one table, so a later build can take them from
   * remote config when Meet changes its page. Labels are English only for now.
   */
  const MEET = {
    leave: 'button[aria-label="Leave call"]',
    input: 'textarea[aria-label^="Send a message" i], textarea[placeholder^="Send a message" i]',
    openChat: 'button[aria-label^="Chat with everyone" i], button[aria-label="Chat" i]',
    send: 'button[aria-label="Send a message" i], button[aria-label="Send" i]',
  };

  /** How long the person may stay in the green room before the notice gives up. */
  const JOIN_LIMIT_MS = 10 * 60_000;
  /** Time for the call to settle after joining, before Anarlog looks for its chat. */
  const SETTLE_MS = 3_000;
  const STEP_MS = 5_000;
  const ATTEMPTS = 3;
  const sentKey = () => `anarlog-meeting-chat:${global.location.pathname}`;

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
  const input = () => {
    const found = document.querySelector(MEET.input);
    return visible(found) ? found : null;
  };

  /** Types into Meet's textarea the way a person does, so its own handlers see the text. */
  function fill(textarea, text) {
    textarea.focus();
    // Meet's handlers read the value through the element's own setter, as typing sets it.
    Reflect.set(global.HTMLTextAreaElement.prototype, "value", text, textarea);
    textarea.dispatchEvent(new InputEvent("input", { bubbles: true, data: text }));
  }

  const enabledSend = () => {
    const button = document.querySelector(MEET.send);
    return button && !button.disabled ? button : null;
  };

  /**
   * Fills Anarlog's text and sends it. Only `sent` and `unconfirmed` follow the send: from then on
   * the message may be in the chat, so the call is marked before it, and never posted in again.
   */
  async function fillAndSend(field, text, live) {
    fill(field, text);
    // Meet took something other than Anarlog's text: take it out, nothing was sent.
    if (field.value !== text) {
      fill(field, "");
      return "not_sent";
    }
    const send = await waitFor(enabledSend, STEP_MS);
    // The person may have typed while Meet's send button was off: send only Anarlog's own text.
    if (field.value !== text) return field.value.trim() ? "draft_in_progress" : "not_sent";
    if (!live()) return "cancelled";
    global.sessionStorage.setItem(sentKey(), "pending");
    if (send) send.click();
    else
      field.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", code: "Enter", keyCode: 13, bubbles: true }),
      );
    // An input that empties is Meet taking the message, not anyone in the call reading it.
    const cleared = await waitFor(() => (field.value.trim() ? null : true), STEP_MS);
    if (!cleared) return "unconfirmed";
    global.sessionStorage.setItem(sentKey(), "sent");
    return "sent";
  }

  /** `live` turns false once the recording stops or a newer post starts. */
  async function attempt(text, live) {
    const focused = document.activeElement;
    let field = input();
    let opened = null;
    if (!field) {
      opened = document.querySelector(MEET.openChat);
      if (!opened) return "chat_unavailable";
      opened.click();
      field = await waitFor(input, STEP_MS);
      if (!field) return "chat_unavailable";
    }
    // Chat turned off by the host, or someone mid-sentence: leave it alone.
    if (field.disabled || field.readOnly) return "chat_unavailable";
    if (field.value.trim()) return "draft_in_progress";
    if (!live()) return "cancelled";
    const outcome = await fillAndSend(field, text, live);
    // Anarlog's text must not stay behind for the person to send by accident.
    if (outcome !== "sent" && field.value === text) fill(field, "");
    // Put the chat and the focus back the way the person had them; a person typing keeps theirs.
    if (
      outcome !== "draft_in_progress" &&
      document.activeElement === field &&
      focused &&
      focused !== field &&
      focused !== document.body &&
      focused.isConnected &&
      typeof focused.focus === "function"
    )
      focused.focus();
    if (outcome === "sent" && opened) opened.click();
    return outcome;
  }

  let current = 0;
  /** Stops a post still waiting to send, for a recording that has stopped. */
  function cancel() {
    current += 1;
  }

  /**
   * Posts `text` once in this call's chat, after the person is in the call. Resolves to `sent`,
   * `already_sent`, `already_attempted` (an earlier send Meet never confirmed), `not_in_call`,
   * `chat_unavailable`, `draft_in_progress`, `not_sent`, `unconfirmed` (sent, but Meet didn't
   * confirm it), or `cancelled`. Only tries that sent nothing go again. The call's mark lasts as
   * long as the call: once the person is out of it, it belongs to a call that has ended, so a later
   * call on the same link gets its own notice.
   */
  async function post(text) {
    if (typeof text !== "string" || !text.trim()) return "not_sent";
    const run = ++current;
    const live = () => run === current;
    const mark = global.sessionStorage.getItem(sentKey());
    if (mark) {
      if (document.querySelector(MEET.leave))
        return mark === "pending" ? "already_attempted" : "already_sent";
      global.sessionStorage.removeItem(sentKey());
    }
    if (!(await waitFor(() => !live() || document.querySelector(MEET.leave), JOIN_LIMIT_MS, 1_000)))
      return "not_in_call";
    await sleep(SETTLE_MS);
    let outcome = "not_sent";
    for (let tries = 0; tries < ATTEMPTS; tries += 1) {
      if (!live()) return "cancelled";
      outcome = await attempt(text, live);
      if (outcome !== "chat_unavailable" && outcome !== "not_sent") return outcome;
      await sleep(STEP_MS);
    }
    return outcome;
  }

  global.anarlogMeetingChat = { MEET, post, cancel, timing: { SETTLE_MS, STEP_MS } };
})(globalThis);
