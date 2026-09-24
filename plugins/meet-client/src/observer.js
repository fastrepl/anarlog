// Injected into the Google Meet webview. Reads participants and speaking
// indicators from the DOM and reports snapshots to the host plugin.
(() => {
  if (window.__anarlogMeet) return;

  let config = window.__ANARLOG_MEET_CONFIG__;
  let lastSerialized = null;
  let lastSentAt = 0;
  let timer = null;
  const HEARTBEAT_MS = 5000;

  const invoke = (command, args) => {
    const internals = window.__TAURI_INTERNALS__;
    if (!internals || typeof internals.invoke !== "function")
      return Promise.resolve();
    return internals.invoke(command, args).catch(() => {});
  };

  const text = (element) =>
    element && element.textContent ? element.textContent.trim() : "";

  const readName = (tile) => {
    const candidates = tile.querySelectorAll(config.name);
    for (const candidate of candidates) {
      const fromAttribute = candidate.getAttribute(config.nameAttribute);
      if (fromAttribute && fromAttribute.trim()) return fromAttribute.trim();
      const fromText = text(candidate);
      if (fromText) return fromText;
    }
    return null;
  };

  const readSpeaking = (tile) => {
    const indicators = tile.querySelectorAll(config.speakingIndicator);
    if (indicators.length === 0) return false;
    for (const indicator of indicators) {
      if (!indicator.classList.contains(config.silentClass)) return true;
    }
    return false;
  };

  const snapshot = () => {
    const byId = new Map();
    for (const tile of document.querySelectorAll(config.tile)) {
      const id = tile.getAttribute(config.participantIdAttribute);
      if (!id) continue;
      const existing = byId.get(id);
      const displayName =
        readName(tile) || (existing && existing.displayName) || null;
      const speaking =
        readSpeaking(tile) || (existing ? existing.speaking : false);
      byId.set(id, { id, displayName, speaking });
    }
    return {
      inMeeting:
        byId.size > 0 || document.querySelector(config.inMeeting) !== null,
      ended: document.querySelector(config.ended) !== null,
      participants: [...byId.values()].sort((a, b) => a.id.localeCompare(b.id)),
    };
  };

  const tick = () => {
    let observation;
    try {
      observation = snapshot();
    } catch (error) {
      return;
    }
    const serialized = JSON.stringify(observation);
    const now = Date.now();
    if (serialized === lastSerialized && now - lastSentAt < HEARTBEAT_MS)
      return;
    lastSerialized = serialized;
    lastSentAt = now;
    invoke("plugin:meet-client|report_observation", { observation });
  };

  const start = () => {
    if (timer) clearInterval(timer);
    timer = setInterval(tick, config.pollMs);
    tick();
  };

  window.__anarlogMeet = {
    configure(next) {
      config = next;
      lastSerialized = null;
      start();
    },
    snapshot,
  };

  if (config) start();
})();
