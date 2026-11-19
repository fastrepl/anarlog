(function installAnarlogMeetingSpeakers(global) {
  "use strict";
  // Another recording in a tab already reading: start over, so its first events say who is lit
  // and who is here now, rather than only what changes from here on.
  if (global.anarlogMeetingSpeakers) {
    global.anarlogMeetingSpeakers.restart();
    return;
  }

  /**
   * Every Google Meet selector this reads, in one table, so a later build can take them from
   * remote config when Meet changes its page. `speakingOutline` is null while Meet's lit tile is
   * told by its look (see `lit`); a selector there replaces that.
   */
  const MEET = {
    tile: "[data-participant-id]",
    tileId: "data-participant-id",
    selfName: "[data-self-name]",
    selfNameAttribute: "data-self-name",
    tileName: "[data-participant-name], [data-name]",
    peopleCount: '[aria-label*="participant" i] [data-count], [data-participant-count]',
    speakingOutline: null,
  };

  /** Reads at most this often; a live call repaints far faster than speakers change. */
  const READ_MS = 200;
  /** The roster again, and proof the tab is still reading, this often. */
  const HEARTBEAT_MS = 5_000;
  /** How deep in a tile its speaking outline may sit. */
  const OUTLINE_DEPTH = 3;

  const clean = (value) => (typeof value === "string" ? value.trim().replace(/\s+/g, " ") : "");

  /** Whether an element draws a visible coloured ring at least 2 px wide. */
  function ringed(element) {
    const style = global.getComputedStyle(element);
    if (style.visibility === "hidden" || style.display === "none" || style.opacity === "0")
      return false;
    const visible = (width, kind, color) =>
      parseFloat(width) >= 2 &&
      kind !== "none" &&
      kind !== "hidden" &&
      color &&
      color !== "transparent" &&
      !/rgba\([^)]*,\s*0\)$/.test(color);
    // A ring drawn as a shadow: `<color> <x> <y> <blur> <spread>`, the spread its width.
    const shadow = style.boxShadow || "none";
    const spread = parseFloat((shadow.match(/-?\d+(?:\.\d+)?px/g) || [])[3]);
    return (
      visible(style.outlineWidth, style.outlineStyle, style.outlineColor) ||
      visible(style.borderTopWidth, style.borderTopStyle, style.borderTopColor) ||
      (shadow !== "none" && spread >= 2 && !/^(transparent|rgba\([^)]*,\s*0\))/.test(shadow))
    );
  }

  /** Whether Meet shows this tile speaking: its outline, on the tile or close inside it. */
  function lit(tile) {
    if (MEET.speakingOutline) return Boolean(tile.querySelector(MEET.speakingOutline));
    const queue = [{ element: tile, depth: 0 }];
    while (queue.length) {
      const { element, depth } = queue.shift();
      if (ringed(element)) return true;
      if (depth < OUTLINE_DEPTH)
        for (const child of element.children) queue.push({ element: child, depth: depth + 1 });
    }
    return false;
  }

  function nameOf(tile, selfName, isSelf) {
    if (isSelf && selfName) return selfName;
    const named = tile.querySelector(MEET.tileName);
    const text =
      clean(named && named.textContent) ||
      clean(tile.getAttribute("aria-label")) ||
      clean((tile.innerText || tile.textContent || "").split("\n").find((line) => line.trim()));
    return text.length <= 120 ? text : "";
  }

  function read() {
    const selfElement = document.querySelector(MEET.selfName);
    const selfName = clean(selfElement && selfElement.getAttribute(MEET.selfNameAttribute));
    const tiles = new Map();
    for (const tile of document.querySelectorAll(MEET.tile)) {
      const id = clean(tile.getAttribute(MEET.tileId));
      if (!id) continue;
      // Meet can show one participant twice, as a camera and as a presentation.
      const seen = tiles.get(id);
      if (seen) {
        seen.lit = seen.lit || lit(tile);
        continue;
      }
      const isSelf = Boolean(tile.querySelector(MEET.selfName));
      const name = nameOf(tile, selfName, isSelf);
      tiles.set(id, {
        participantId: id,
        name,
        isSelf: isSelf || (Boolean(selfName) && name === selfName),
        lit: lit(tile),
      });
    }
    const countText = clean(document.querySelector(MEET.peopleCount)?.textContent);
    const count = /^\d+$/.test(countText) ? Number(countText) : null;
    return { tiles: [...tiles.values()], count };
  }

  const meetingCode = () =>
    (/^\/(?:meet\/)?([a-z]{3}-[a-z]{4}-[a-z]{3})$/.exec(location.pathname) || [])[1] || null;

  const pending = [];
  let speaker;
  let roster = "";
  let lastHeartbeat = 0;
  let timer;
  /** Each start's own number, so a send that fails after a restart can't stop the new reader. */
  let run = 0;
  /** The call this start reads; Meet moving the tab to another call ends it. */
  let call = null;

  function tick() {
    const now = Date.now();
    if (meetingCode() !== call) {
      stop();
      return;
    }
    let seen;
    try {
      seen = read();
    } catch {
      return;
    }
    const litIds = seen.tiles.filter((tile) => tile.lit).map((tile) => tile.participantId);
    // While two tiles are lit, the one already speaking stays: Meet often lights a second voice
    // for a moment.
    const active = litIds.includes(speaker) ? speaker : (litIds[0] ?? null);
    if (active !== speaker) {
      speaker = active;
      const tile = seen.tiles.find((entry) => entry.participantId === active);
      pending.push({ kind: "speaker", at: now, participantId: active, name: tile?.name ?? null });
    }
    const self = seen.tiles.find((tile) => tile.isSelf);
    const snapshot = {
      kind: "roster",
      selfId: self ? self.participantId : null,
      participants: seen.tiles.map(({ participantId, name }) => ({ participantId, name })),
      count: seen.count,
    };
    const key = JSON.stringify(snapshot);
    const beat = now - lastHeartbeat >= HEARTBEAT_MS;
    if (key !== roster || beat) {
      roster = key;
      pending.push({ ...snapshot, at: now });
    }
    if (beat) {
      lastHeartbeat = now;
      pending.push({ kind: "heartbeat", at: now, meetingCode: call });
    }
    if (pending.length === 0) return;
    const events = pending.splice(0);
    // The extension was reloaded or removed: nothing is listening any more. Chrome says so by
    // rejecting the send, or, in a tab the old extension left behind, by throwing at once.
    const sent = run;
    const gone = () => {
      if (sent === run) stop();
    };
    try {
      void Promise.resolve(
        chrome.runtime.sendMessage({ type: "anarlog_meeting_speakers", events }),
      ).catch(gone);
    } catch {
      gone();
    }
  }

  /** Stops reading, as when the tab lets go of the recording; safe to call again. */
  function stop() {
    clearInterval(timer);
    timer = undefined;
    run += 1;
    pending.length = 0;
    speaker = undefined;
    roster = "";
    lastHeartbeat = 0;
  }

  function restart() {
    stop();
    call = meetingCode();
    timer = setInterval(tick, READ_MS);
    tick();
  }

  global.anarlogMeetingSpeakers = { MEET, read, restart, stop };
  restart();
})(globalThis);
