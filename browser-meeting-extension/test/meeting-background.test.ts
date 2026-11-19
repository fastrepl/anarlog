import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import vm from "node:vm";

import { afterEach, describe, expect, it, vi } from "vitest";

type Message = {
  type?: string;
  event?: string;
  url?: string;
  message?: string;
  events?: unknown[];
  notice?: string;
  camera?: string;
  outcome?: string;
  pausable?: boolean;
};
type Listener = (message: Message) => void;
type Tab = { id: number; url: string };
type Sender = { tab: { id: number }; url: string };
type Controller = {
  click(info: { menuItemId: string }): void;
  updated(id: number, change: { url?: string; status?: string }): void;
  removed(id: number): void;
  toolbar(tab: Tab): void;
  ended(sender: Sender): void;
  speakers(sender: Sender, events: unknown): void;
};
type Injection = { target: { tabId: number }; files?: string[]; func?: unknown; args?: unknown[] };

const MEET = "https://meet.google.com/abc-defg-hij";
const HOST = "so.anarlog.browser_meeting";

/**
 * `scriptResult` is what an injected function resolves to in the page; `unreadable` makes the page
 * scripts' injection fail, as it does without the click's activeTab grant.
 */
function setup(withScripting = false, scriptResult?: unknown, unreadable = false) {
  const source = readFileSync(
    resolve(import.meta.dirname, "../extension/meeting-background.js"),
    "utf8",
  );
  // The background's own globals, which an injected function sees as the page's.
  const page: Record<string, unknown> = { URL, setTimeout, clearTimeout };
  const module = vm.runInNewContext(`${source}\nAnarlogMeetingBackground`, page);
  const commands: Message[] = [];
  const badges: string[] = [];
  const titles: string[] = [];
  const hosts: string[] = [];
  const injections: Injection[] = [];
  let disconnects = 0;
  let message: Listener = () => {};
  let disconnect: () => void = () => {};
  const api = {
    ...(withScripting
      ? {
          scripting: {
            executeScript: async (injection: Injection) => {
              injections.push(injection);
              if (unreadable && injection.files?.includes("meeting-content.js"))
                throw new Error("Cannot access contents of the page.");
              const result =
                typeof scriptResult === "function" ? scriptResult(injection) : scriptResult;
              return injection.func ? [{ result }] : undefined;
            },
          },
        }
      : {}),
    action: {
      setBadgeText: ({ text }: { text: string }) => void badges.push(text),
      setTitle: ({ title }: { title: string }) => void titles.push(title),
    },
    runtime: {
      connectNative: (host: string) => {
        hosts.push(host);
        return {
          postMessage: (value: Message) => void commands.push(value),
          disconnect: () => void (disconnects += 1),
          onMessage: { addListener: (fn: Listener) => void (message = fn) },
          onDisconnect: { addListener: (fn: () => void) => void (disconnect = fn) },
        };
      },
    },
  };
  const controller: Controller = module.createController(api, HOST);
  const start = (url = "https://meet.google.com/meet/abc-defg-hij?authuser=1", id = 4) =>
    controller.toolbar({ id, url });
  const receive = (value: Message) => message(value);
  return {
    controller,
    start,
    commands,
    badges,
    titles,
    hosts,
    injections,
    receive,
    /** The app's answer to a start: the host reached it, then the recording began. */
    started: (extra: Message = {}) => {
      receive({ event: "connected" });
      receive({ event: "started", pausable: true, ...extra });
    },
    disconnect: () => disconnect(),
    disconnects: () => disconnects,
    module,
    page,
  };
}

const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

afterEach(() => vi.useRealTimers());

describe("Meet recording controls", () => {
  it("starts one recording in Anarlog, from a Meet call only, without the call's query values", () => {
    const s = setup();
    for (const url of [
      "https://example.com",
      "https://meet.google.com/home",
      "https://meet.google.com.evil/abc-defg-hij",
      "http://meet.google.com/abc-defg-hij",
    ])
      s.start(url);
    expect(s.hosts).toEqual([]);
    s.start();
    s.start("https://meet.google.com/xxx-yyyy-zzz", 9);
    expect(s.hosts).toEqual([HOST]);
    expect(s.commands).toEqual([{ type: "meeting_start", url: MEET }]);
    expect(s.badges.at(-1)).toBe("…");
    s.started();
    expect(s.titles.at(-1)).toContain("Anarlog is recording this call");
  });

  it("gives up on a host that never reaches Anarlog, as an old one doesn't", () => {
    vi.useFakeTimers();
    const s = setup();
    s.start();
    vi.advanceTimersByTime(s.module.CONNECT_TIMEOUT_MS);
    expect(s.badges.at(-1)).toBe("!");
    expect(s.titles.at(-1)).toBe("Update Anarlog and the extension together.");
    expect(s.disconnects()).toBe(1);
    s.start();
    expect(s.hosts).toHaveLength(2);

    const answered = setup();
    answered.start();
    answered.receive({ event: "connected" });
    vi.advanceTimersByTime(answered.module.CONNECT_TIMEOUT_MS * 2);
    expect(answered.disconnects()).toBe(0);
  });

  it("posts the notice only after the camera switch is done, and reports each once", async () => {
    const s = setup(true, (injection: Injection) =>
      injection.args?.[0] === "Anarlog Camera" ? "switched" : "sent",
    );
    s.start();
    const pageScripts = s.injections.length;
    s.started({ notice: "Anarlog is taking notes.", camera: "Anarlog Camera" });
    await tick();
    expect(
      s.injections
        .slice(pageScripts)
        .map((injection) => [injection.target.tabId, injection.files, injection.args]),
    ).toEqual([
      [4, ["meeting-camera.js"], undefined],
      [4, undefined, ["Anarlog Camera"]],
      [4, ["meeting-chat.js"], undefined],
      [4, undefined, ["Anarlog is taking notes."]],
    ]);
    expect(s.commands.filter((c) => c.type === "meeting_camera" || c.type === "meeting_chat"))
      .toEqual([
        { type: "meeting_camera", outcome: "switched" },
        { type: "meeting_chat", outcome: "sent" },
      ]);
    s.receive({ event: "paused" });
    s.receive({ event: "resumed" });
    await tick();
    expect(s.injections.slice(pageScripts)).toHaveLength(4);

    const quiet = setup(true, "sent");
    quiet.start();
    quiet.started();
    await tick();
    expect(quiet.injections.flatMap((injection) => injection.files ?? [])).toEqual([
      "meeting-content.js",
      "meeting-speakers.js",
    ]);
  });

  it("tells Anarlog nothing about a notice once its link ended before the post finished", async () => {
    const late = setup(true, "sent");
    late.start();
    late.started({ notice: "Anarlog is taking notes." });
    late.receive({ event: "detached" });
    await tick();
    expect(late.commands.map((command) => command.type)).not.toContain("meeting_chat");
  });

  it("pauses and resumes on the toolbar only when the recorder can pause", () => {
    const s = setup();
    const tab = { id: 4, url: MEET };
    s.start();
    s.started();
    s.controller.toolbar({ ...tab, id: 9 });
    expect(s.commands.at(-1)?.type).toBe("meeting_start");
    s.controller.toolbar(tab);
    expect(s.commands.at(-1)).toEqual({ type: "meeting_pause" });
    s.receive({ event: "paused" });
    expect(s.titles.at(-1)).toContain("to resume");
    s.controller.toolbar(tab);
    expect(s.commands.at(-1)).toEqual({ type: "meeting_resume" });
    // Anarlog refused: the recording is as it was, and the person hears why.
    s.receive({ event: "refused", message: "Anarlog couldn't change the recording." });
    expect(s.titles.at(-1)).toBe("Anarlog couldn't change the recording.");
    s.controller.toolbar(tab);
    expect(s.commands.at(-1)).toEqual({ type: "meeting_resume" });
    s.receive({ event: "resumed" });
    expect(s.titles.at(-1)).toContain("to pause");

    const fixed = setup();
    fixed.start();
    fixed.started({ pausable: false });
    fixed.controller.toolbar(tab);
    expect(fixed.commands.map((command) => command.type)).toEqual(["meeting_start"]);
    expect(fixed.titles.at(-1)).toContain("Stop it from this button's menu");
  });

  it("follows a pause made in Anarlog's own window", () => {
    const s = setup();
    s.start();
    s.started();
    s.receive({ event: "paused" });
    expect(s.titles.at(-1)).toContain("to resume");
    s.receive({ event: "resumed" });
    expect(s.titles.at(-1)).toContain("to pause");
  });

  it("lets go once when the tab navigates, closes, or shows the call ended, never stopping", () => {
    const s = setup();
    const sender = { tab: { id: 4 }, url: MEET };
    s.start();
    s.started();
    s.controller.removed(9);
    s.controller.ended({ tab: { id: 9 }, url: MEET });
    s.controller.ended({ tab: { id: 4 }, url: "https://example.com" });
    s.controller.updated(4, { url: "https://meet.google.com/abc-defg-hij?authuser=1" });
    expect(s.commands.map((command) => command.type)).toEqual(["meeting_start"]);
    s.controller.ended(sender);
    s.controller.updated(4, { status: "loading" });
    s.controller.removed(4);
    expect(s.commands.map((command) => command.type)).toEqual(["meeting_start", "meeting_detach"]);
    s.receive({ event: "ended" });
    expect(s.badges.at(-1)).toBe("");
    expect(s.titles.at(-1)).toBe("Recording saved in Anarlog.");
  });

  it("keeps letting go when a start lands after the Meet tab closed", () => {
    const s = setup();
    s.start();
    s.controller.removed(4);
    s.started();
    expect(s.titles.at(-1)).toBe("Letting go of this tab…");
    expect(s.commands.at(-1)).toEqual({ type: "meeting_detach" });
  });

  it("leaves Anarlog's recording going when a joined tab goes, and a fresh click links again", () => {
    const s = setup();
    s.start();
    s.started();
    s.controller.updated(4, { url: "https://meet.google.com/xxx-yyyy-zzz" });
    s.receive({ event: "detached" });
    expect(s.titles.at(-1)).toBe("Anarlog is still recording in the app.");
    s.start();
    expect(s.hosts).toHaveLength(2);
  });

  it("lets go of a Meet tab it can't read, rather than stopping Anarlog's recording", async () => {
    const s = setup(true, undefined, true);
    s.start();
    await tick();
    expect(s.commands.map((command) => command.type)).toEqual(["meeting_start", "meeting_detach"]);
  });

  it("shows Anarlog's refusal or a lost connection, then lets a fresh click start again", () => {
    const s = setup();
    s.start();
    s.receive({ event: "connected" });
    s.receive({ event: "error", message: "Anarlog is recording a different Meet call." });
    expect(s.badges.at(-1)).toBe("!");
    expect(s.titles.at(-1)).toBe("Anarlog is recording a different Meet call.");
    s.start();
    s.started();
    s.disconnect();
    expect(s.badges.at(-1)).toBe("!");
    s.start();
    expect(s.hosts).toHaveLength(3);
  });

  it("stops only on the person's own Stop", () => {
    const s = setup();
    s.start();
    s.started();
    s.controller.click({ menuItemId: "anarlog-meeting-stop" });
    s.controller.click({ menuItemId: "anarlog-meeting-stop" });
    expect(s.commands.map((command) => command.type)).toEqual(["meeting_start", "meeting_stop"]);
  });

  it("stops the tab's page scripts once the link is over, so a later click starts them again", async () => {
    const s = setup(true);
    s.start();
    s.started();
    s.receive({ event: "detached" });
    await tick();
    const quiet = s.injections.at(-1);
    expect(quiet?.target.tabId).toBe(4);
    const stopped: string[] = [];
    const names = [
      "anarlogMeetingChat",
      "anarlogMeetingCamera",
      "anarlogMeetingSpeakers",
      "anarlogMeetingMonitor",
    ];
    for (const name of names)
      s.page[name] = {
        cancel: () => stopped.push(`${name}.cancel`),
        stop: () => stopped.push(`${name}.stop`),
      };
    (quiet!.func as () => void)();
    expect(stopped).toEqual([
      "anarlogMeetingChat.cancel",
      "anarlogMeetingCamera.cancel",
      "anarlogMeetingSpeakers.stop",
      "anarlogMeetingMonitor.stop",
    ]);
    s.start();
    await tick();
    expect(s.injections.at(-1)?.files).toEqual(["meeting-content.js", "meeting-speakers.js"]);
  });

  it("passes on who Meet shows speaking from the recording tab only, until the stop", () => {
    const s = setup();
    const events = [{ kind: "speaker", at: 1, participantId: "p-alice", name: "Alice" }];
    const sender = { tab: { id: 4 }, url: MEET };
    s.controller.speakers(sender, events);
    expect(s.commands).toEqual([]);
    s.start();
    s.controller.speakers({ tab: { id: 9 }, url: MEET }, events);
    s.controller.speakers({ tab: { id: 4 }, url: "https://meet.google.com/xxx-yyyy-zzz" }, events);
    s.controller.speakers(sender, "not a list");
    s.controller.speakers(sender, events);
    s.controller.click({ menuItemId: "anarlog-meeting-stop" });
    s.controller.speakers(sender, events);
    expect(s.commands.map((command) => command.type)).toEqual([
      "meeting_start",
      "meeting_cues",
      "meeting_stop",
    ]);
    expect(s.commands[1]).toEqual({ type: "meeting_cues", events });
  });
});
