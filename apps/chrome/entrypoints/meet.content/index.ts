import "~/assets/tailwind.css";

import {
  getMuteState,
  getParticipantNames,
  isMeetPageActive,
  type Participant,
} from "./dom";

const SEND_INTERVAL_MS = 2500;
const MUTATION_DEBOUNCE_MS = 250;
const BADGE_ANCHOR_SELECTOR =
  "[role='toolbar'], [aria-label*='Call controls' i], footer";

type MeetingStateMessage = {
  type: "meeting_state";
  url: string;
  is_active: true;
  muted: boolean;
  participants: Participant[];
};

type MeetingEndedMessage = {
  type: "meeting_ended";
  url: string;
  is_active: false;
};

type HostMessage = MeetingStateMessage | MeetingEndedMessage;

let badgeElement: HTMLDivElement | null = null;

function buildMeetingStatePayload(): MeetingStateMessage {
  return {
    type: "meeting_state",
    url: window.location.href,
    is_active: true,
    muted: getMuteState(document),
    participants: getParticipantNames(document),
  };
}

function buildMeetingEndedPayload(): MeetingEndedMessage {
  return {
    type: "meeting_ended",
    url: window.location.href,
    is_active: false,
  };
}

function getBadgeClass(isConnected: boolean) {
  const stateClass = isConnected
    ? "border-[#2c7a5e] bg-[#0f2f26] text-[#8af8c5]"
    : "border-[#9a6a34] bg-[#302012] text-[#f9c786]";

  return [
    "whitespace-nowrap rounded-full border px-3 py-2 text-xs leading-none tracking-[0.01em]",
    "pointer-events-none mr-[10px] self-center font-[IBM_Plex_Sans] transition-colors duration-150",
    stateClass,
  ].join(" ");
}

function updateBadge(isConnected: boolean) {
  if (!badgeElement) {
    return;
  }

  badgeElement.className = getBadgeClass(isConnected);
  badgeElement.textContent = isConnected
    ? "Char: connected"
    : "Char: disconnected";
}

async function sendPayload(payload: HostMessage) {
  try {
    const response = (await browser.runtime.sendMessage(payload)) as
      | { ok?: boolean }
      | undefined;
    updateBadge(Boolean(response?.ok));
  } catch (error) {
    console.warn("[char] failed to send message to background:", error);
    updateBadge(false);
  }
}

export default defineContentScript({
  matches: ["https://meet.google.com/*"],
  cssInjectionMode: "ui",
  runAt: "document_idle",
  async main(ctx) {
    const ui = await createShadowRootUi(ctx, {
      name: "char-meet-badge",
      position: "inline",
      anchor: BADGE_ANCHOR_SELECTOR,
      append: "first",
      onMount: (container) => {
        badgeElement = document.createElement("div");
        container.append(badgeElement);
        updateBadge(false);
        return badgeElement;
      },
      onRemove: () => {
        badgeElement = null;
      },
    });

    ui.autoMount();

    let lastPayloadKey = "";
    let lastSentAt = 0;
    let mutationTimeout: number | null = null;

    const maybeSendState = (force = false) => {
      if (!isMeetPageActive(window.location, document)) {
        if (force) {
          void sendPayload(buildMeetingEndedPayload());
        }
        return;
      }

      const payload = buildMeetingStatePayload();
      const payloadKey = JSON.stringify(payload);
      const now = Date.now();
      const shouldSend =
        force ||
        payloadKey !== lastPayloadKey ||
        now - lastSentAt >= SEND_INTERVAL_MS;

      if (!shouldSend) {
        return;
      }

      lastPayloadKey = payloadKey;
      lastSentAt = now;
      void sendPayload(payload);
    };

    const observer = new MutationObserver(() => {
      if (mutationTimeout !== null) {
        return;
      }

      mutationTimeout = window.setTimeout(() => {
        mutationTimeout = null;
        maybeSendState();
      }, MUTATION_DEBOUNCE_MS);
    });

    observer.observe(document.documentElement, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["aria-label", "data-is-muted", "aria-pressed"],
    });

    maybeSendState(true);

    ctx.setInterval(() => {
      maybeSendState();
    }, SEND_INTERVAL_MS);

    ctx.addEventListener(window, "beforeunload", () => {
      if (mutationTimeout !== null) {
        window.clearTimeout(mutationTimeout);
      }
      void sendPayload(buildMeetingEndedPayload());
    });

    ctx.addEventListener(window, "wxt:locationchange", () => {
      maybeSendState(true);
    });
  },
});
