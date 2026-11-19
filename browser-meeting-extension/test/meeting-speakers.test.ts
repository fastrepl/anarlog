// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import vm from "node:vm";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

type Event = Record<string, unknown> & { kind: string };

const LIT = "border: 4px solid rgb(26, 115, 232)";

/** A Meet call's grid, shaped as Meet marks it: one tile per participant, the person's own named. */
function call(lit: string | null, count = 3) {
  const tile = (id: string, name: string, self = false) => `
    <div data-participant-id="${id}">
      <div class="frame" style="${lit === id ? LIT : "border: 0"}"></div>
      <div class="name">${self ? `<span data-self-name="John Jeong">You</span>` : name}</div>
    </div>`;
  document.body.innerHTML = `
    <main>
      ${tile("spaces/s/devices/1", "", true)}
      ${tile("spaces/s/devices/2", "Alice Kim")}
      ${tile("spaces/s/devices/3", "Bob Lee")}
      <button aria-label="Show everyone, 3 participants"><span data-count>${count}</span></button>
    </main>`;
}

function start() {
  const sent: Event[] = [];
  const sendMessage = vi.fn((message: { type: string; events: Event[] }) => {
    expect(message.type).toBe("anarlog_meeting_speakers");
    sent.push(...message.events);
    return Promise.resolve();
  });
  const script = readFileSync(
    resolve(import.meta.dirname, "../extension/meeting-speakers.js"),
    "utf8",
  );
  const location = { pathname: "/abc-defg-hij" };
  const tab = vm.createContext({
    document,
    location,
    getComputedStyle: (element: Element) => window.getComputedStyle(element),
    setInterval,
    clearInterval,
    Date,
    Promise,
    JSON,
    chrome: { runtime: { sendMessage } },
  });
  // Each recording injects the script into the tab again.
  const inject = () => vm.runInContext(script, tab);
  inject();
  const stop = () => vm.runInContext("anarlogMeetingSpeakers.stop()", tab);
  return { sent, sendMessage, inject, stop, location };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

it("tells who Meet lights as speaking, the roster with the person's own tile, and a heartbeat", () => {
  call(null);
  const { sent } = start();
  expect(sent.map((event) => event.kind)).toEqual([
    "speaker",
    "roster",
    "heartbeat",
  ]);
  expect(sent[0]).toMatchObject({ participantId: null });
  expect(sent[1]).toMatchObject({
    selfId: "spaces/s/devices/1",
    count: 3,
    participants: [
      { participantId: "spaces/s/devices/1", name: "John Jeong" },
      { participantId: "spaces/s/devices/2", name: "Alice Kim" },
      { participantId: "spaces/s/devices/3", name: "Bob Lee" },
    ],
  });
  expect(sent[2]).toMatchObject({ meetingCode: "abc-defg-hij" });

  sent.length = 0;
  call("spaces/s/devices/2");
  vi.advanceTimersByTime(200);
  call("spaces/s/devices/3");
  vi.advanceTimersByTime(200);
  // Unchanged reads say nothing until the next heartbeat.
  vi.advanceTimersByTime(1_000);
  expect(sent).toEqual([
    expect.objectContaining({
      kind: "speaker",
      participantId: "spaces/s/devices/2",
      name: "Alice Kim",
    }),
    expect.objectContaining({
      kind: "speaker",
      participantId: "spaces/s/devices/3",
      name: "Bob Lee",
    }),
  ]);

  sent.length = 0;
  vi.advanceTimersByTime(5_000);
  expect(sent.map((event) => event.kind)).toEqual(["roster", "heartbeat"]);
});

it("reads no more often than every 200 ms however fast Meet repaints", () => {
  call(null);
  const read = vi.spyOn(document, "querySelectorAll");
  start();
  read.mockClear();
  for (let step = 0; step < 10; step++) {
    call(step % 2 ? "spaces/s/devices/2" : null);
    vi.advanceTimersByTime(20);
  }
  expect(
    read.mock.calls.filter(
      ([selector]) => selector === "[data-participant-id]",
    ),
  ).toHaveLength(1);
});

it("keeps the speaker already lit while Meet briefly lights a second tile", () => {
  call("spaces/s/devices/2");
  const { sent } = start();
  sent.length = 0;
  const bob = document.querySelector(
    '[data-participant-id="spaces/s/devices/3"] .frame',
  );
  bob?.setAttribute("style", LIT);
  vi.advanceTimersByTime(200);
  expect(sent.filter((event) => event.kind === "speaker")).toEqual([]);
});

it("tells the next recording in the same tab who is lit and who is here, from its start", () => {
  call("spaces/s/devices/2");
  const { sent, inject } = start();
  vi.advanceTimersByTime(1_000);
  sent.length = 0;

  inject();

  expect(sent.map((event) => event.kind)).toEqual([
    "speaker",
    "roster",
    "heartbeat",
  ]);
  expect(sent[0]).toMatchObject({
    participantId: "spaces/s/devices/2",
    name: "Alice Kim",
  });
  sent.length = 0;
  vi.advanceTimersByTime(1_000);
  // One reader still: unchanged reads say nothing.
  expect(sent).toEqual([]);
});

it("stops reading once the extension is gone", async () => {
  call(null);
  const { sendMessage } = start();
  sendMessage.mockImplementation(() =>
    Promise.reject(new Error("Extension context invalidated.")),
  );
  call("spaces/s/devices/2");
  vi.advanceTimersByTime(200);
  await vi.advanceTimersByTimeAsync(0);
  sendMessage.mockClear();
  call("spaces/s/devices/3");
  vi.advanceTimersByTime(1_000);
  expect(sendMessage).not.toHaveBeenCalled();
});

it("stops reading, without throwing on every read, when Chrome refuses the send outright", () => {
  call(null);
  const { sendMessage } = start();
  // An orphaned content script's `sendMessage` throws at once rather than rejecting.
  sendMessage.mockImplementation(() => {
    throw new Error("Extension context invalidated.");
  });
  call("spaces/s/devices/2");
  expect(() => vi.advanceTimersByTime(200)).not.toThrow();
  sendMessage.mockClear();
  call("spaces/s/devices/3");
  vi.advanceTimersByTime(1_000);
  expect(sendMessage).not.toHaveBeenCalled();
});

it("stops reading when the tab lets go, and reads afresh when it links again", () => {
  call("spaces/s/devices/2");
  const { sent, sendMessage, inject, stop } = start();
  stop();
  stop();
  sendMessage.mockClear();
  call("spaces/s/devices/3");
  vi.advanceTimersByTime(10_000);
  expect(sendMessage).not.toHaveBeenCalled();

  sent.length = 0;
  inject();
  expect(sent.map((event) => event.kind)).toEqual([
    "speaker",
    "roster",
    "heartbeat",
  ]);
  expect(sent[0]).toMatchObject({
    participantId: "spaces/s/devices/3",
    name: "Bob Lee",
  });
});

it("keeps reading after a relink when a send from before it fails late", async () => {
  call(null);
  const { sent, sendMessage, inject } = start();
  let fail: (error: Error) => void = () => {};
  sendMessage.mockImplementationOnce(
    (message: { type: string; events: Event[] }) =>
      new Promise((_, reject) => {
        sent.push(...message.events);
        fail = reject;
      }),
  );
  call("spaces/s/devices/2");
  vi.advanceTimersByTime(200);
  inject();
  fail(new Error("Could not establish connection."));
  await vi.advanceTimersByTimeAsync(0);
  sent.length = 0;
  call("spaces/s/devices/3");
  vi.advanceTimersByTime(200);
  expect(sent).toEqual([
    expect.objectContaining({
      kind: "speaker",
      participantId: "spaces/s/devices/3",
    }),
  ]);
});

it("stops reading once Meet moves the tab to another call", () => {
  call(null);
  const { sendMessage, location } = start();
  location.pathname = "/xxx-yyyy-zzz";
  sendMessage.mockClear();
  call("spaces/s/devices/2");
  vi.advanceTimersByTime(10_000);
  expect(sendMessage).not.toHaveBeenCalled();
});
