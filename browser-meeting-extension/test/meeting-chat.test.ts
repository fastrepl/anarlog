// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import vm from "node:vm";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const source = readFileSync(
  resolve(import.meta.dirname, "../extension/meeting-chat.js"),
  "utf8",
);

type MeetingChat = { post(text: string): Promise<string>; cancel(): void };

/** A Meet call page: in the call, with its chat panel closed and opening on the chat button. */
function meetPage(
  options: {
    inCall?: boolean;
    chatOn?: boolean;
    draft?: string;
    sendOff?: boolean;
    /** How long Meet takes to empty its input after a send; null for never. */
    clearsAfterMs?: number | null;
  } = {},
) {
  document.body.innerHTML = `
    ${options.inCall === false ? "" : '<button aria-label="Leave call"></button>'}
    <button aria-label="Chat with everyone"></button>
    <input id="elsewhere">
    <div id="panel"></div>`;
  const sent: string[] = [];
  const keys: string[] = [];
  document.addEventListener("keydown", (event) => keys.push(event.key));
  const panel = document.getElementById("panel")!;
  const chatButton = document.querySelector<HTMLButtonElement>(
    '[aria-label="Chat with everyone"]',
  )!;
  chatButton.addEventListener("click", () => {
    if (panel.childElementCount) return void (panel.innerHTML = "");
    panel.innerHTML = `<textarea aria-label="Send a message"></textarea>
      <button aria-label="Send a message" disabled></button>`;
    const textarea = panel.querySelector("textarea")!;
    const send = panel.querySelector("button")!;
    textarea.disabled = options.chatOn === false;
    textarea.value = options.draft ?? "";
    textarea.addEventListener(
      "input",
      () => (send.disabled = options.sendOff === true || !textarea.value),
    );
    send.addEventListener("click", () => {
      sent.push(textarea.value);
      const clears =
        options.clearsAfterMs === undefined ? 0 : options.clearsAfterMs;
      if (clears === 0) textarea.value = "";
      else if (clears !== null) setTimeout(() => (textarea.value = ""), clears);
    });
  });
  return { sent, panel, keys };
}

function install(): MeetingChat {
  const context = {
    document,
    location: window.location,
    sessionStorage: window.sessionStorage,
    HTMLTextAreaElement: window.HTMLTextAreaElement,
    InputEvent: window.InputEvent,
    KeyboardEvent: window.KeyboardEvent,
    setTimeout,
    // The faked clock, not the context's own.
    Date,
    globalThis: undefined as unknown,
  };
  context.globalThis = context;
  vm.runInNewContext(source, context);
  return (context as unknown as { anarlogMeetingChat: MeetingChat })
    .anarlogMeetingChat;
}

async function post(chat: MeetingChat, text: string) {
  const result = chat.post(text);
  // Past the ten minutes the notice waits for the person to join.
  await vi.advanceTimersByTimeAsync(11 * 60_000);
  return result;
}

beforeEach(() => {
  vi.useFakeTimers();
  window.sessionStorage.clear();
});
afterEach(() => vi.useRealTimers());

it("opens the chat, sends the notice once, and closes the chat again", async () => {
  const page = meetPage();
  const chat = install();

  expect(await post(chat, "Anarlog is taking notes.")).toBe("sent");
  expect(page.sent).toEqual(["Anarlog is taking notes."]);
  expect(page.panel.childElementCount).toBe(0);
  expect(await post(install(), "Anarlog is taking notes.")).toBe(
    "already_sent",
  );
  expect(page.sent).toHaveLength(1);
});

it("leaves a person's draft alone", async () => {
  const page = meetPage({ draft: "one sec" });
  expect(await post(install(), "Anarlog is taking notes.")).toBe(
    "draft_in_progress",
  );
  expect(page.sent).toEqual([]);
});

it("sends nothing while the host has chat off", async () => {
  const page = meetPage({ chatOn: false });
  expect(await post(install(), "Anarlog is taking notes.")).toBe(
    "chat_unavailable",
  );
  expect(page.sent).toEqual([]);
});

it("gives up when the person never joins the call", async () => {
  const page = meetPage({ inCall: false });
  expect(await post(install(), "Anarlog is taking notes.")).toBe("not_in_call");
  expect(page.sent).toEqual([]);
});

it("posts again in a later call on the same link, after the earlier call ended", async () => {
  meetPage();
  expect(await post(install(), "Anarlog is taking notes.")).toBe("sent");
  // The next call on this link starts in the green room, and the person joins after a while.
  const page = meetPage({ inCall: false });
  const result = install().post("Anarlog is taking notes.");
  await vi.advanceTimersByTimeAsync(5_000);
  document.body.insertAdjacentHTML(
    "afterbegin",
    '<button aria-label="Leave call"></button>',
  );
  await vi.advanceTimersByTimeAsync(11 * 60_000);
  expect(await result).toBe("sent");
  expect(page.sent).toEqual(["Anarlog is taking notes."]);
});

it("sends nothing once the recording stops before the person joins", async () => {
  const page = meetPage({ inCall: false });
  const chat = install();
  const result = chat.post("Anarlog is taking notes.");
  await vi.advanceTimersByTimeAsync(5_000);
  chat.cancel();
  document.body.insertAdjacentHTML(
    "afterbegin",
    '<button aria-label="Leave call"></button>',
  );
  await vi.advanceTimersByTimeAsync(11 * 60_000);
  expect(await result).toBe("cancelled");
  expect(page.sent).toEqual([]);
});

it("leaves the field alone when the person types while Meet's send button is off", async () => {
  const page = meetPage({ sendOff: true });
  const result = install().post("Anarlog is taking notes.");
  // In the call and the chat opened: the notice is in the field, waiting on the send button.
  await vi.advanceTimersByTimeAsync(4_000);
  const field = page.panel.querySelector("textarea")!;
  expect(field.value).toBe("Anarlog is taking notes.");
  field.value = "Anarlog is taking notes. Also, wait";
  await vi.advanceTimersByTimeAsync(11 * 60_000);
  expect(await result).toBe("draft_in_progress");
  expect(page.keys).toEqual([]);
  expect(field.value).toBe("Anarlog is taking notes. Also, wait");
});

it("never sends again after a send Meet confirms too late", async () => {
  // Meet takes the message, but empties its input only after Anarlog stopped waiting.
  const page = meetPage({ clearsAfterMs: 8_000 });
  expect(await post(install(), "Anarlog is taking notes.")).toBe("unconfirmed");
  expect(page.sent).toEqual(["Anarlog is taking notes."]);
});

it("answers a second start in the same call without sending again", async () => {
  // A reconnect, or the extension's background restarting, posts again mid-call.
  const page = meetPage({ clearsAfterMs: null });
  expect(await post(install(), "Anarlog is taking notes.")).toBe("unconfirmed");
  expect(await post(install(), "Anarlog is taking notes.")).toBe(
    "already_attempted",
  );
  expect(page.sent).toEqual(["Anarlog is taking notes."]);
  // Anarlog's text didn't stay in the box for the person to send by accident.
  expect(page.panel.querySelector("textarea")!.value).toBe("");
});

it("takes the notice back out when the recording stops while Meet's send button is off", async () => {
  const page = meetPage({ sendOff: true });
  const chat = install();
  const result = chat.post("Anarlog is taking notes.");
  await vi.advanceTimersByTimeAsync(4_000);
  const field = page.panel.querySelector("textarea")!;
  expect(field.value).toBe("Anarlog is taking notes.");
  chat.cancel();
  await vi.advanceTimersByTimeAsync(11 * 60_000);
  expect(await result).toBe("cancelled");
  expect(field.value).toBe("");
  expect(page.sent).toEqual([]);
  expect(page.keys).toEqual([]);
});

it("gives the focus back to where the person had it", async () => {
  meetPage();
  const elsewhere = document.getElementById("elsewhere") as HTMLInputElement;
  elsewhere.focus();
  expect(await post(install(), "Anarlog is taking notes.")).toBe("sent");
  expect(document.activeElement).toBe(elsewhere);
});
