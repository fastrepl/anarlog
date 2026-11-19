// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import vm from "node:vm";

import { afterEach, beforeEach, expect, it, vi } from "vitest";

const source = readFileSync(resolve(import.meta.dirname, "../extension/meeting-camera.js"), "utf8");

type MeetingCamera = { select(name: string): Promise<string>; cancel(): void };

/**
 * A Meet call page whose More options menu opens Settings, with a Video tab holding a camera
 * picker that lists `cameras` and shows the one in use in its label.
 */
function meetPage(options: { inCall?: boolean; cameras?: string[]; current?: string } = {}) {
  const cameras = options.cameras ?? ["MacBook Pro Camera", "Anarlog Camera"];
  let current = options.current ?? cameras[0];
  document.body.innerHTML = `
    ${options.inCall === false ? "" : '<button aria-label="Leave call"></button>'}
    <button aria-label="Turn off camera"></button>
    <button aria-label="More options"></button>
    <div id="layer"></div>`;
  const layer = document.getElementById("layer")!;
  const picks: string[] = [];
  document.querySelector('[aria-label="More options"]')!.addEventListener("click", () => {
    layer.innerHTML = '<ul role="menu"><li role="menuitem">Settings</li></ul>';
    layer.querySelector('[role="menuitem"]')!.addEventListener("click", () => {
      layer.innerHTML = `<div role="dialog">
        <div role="tab">Audio</div><div role="tab">Video</div>
        <div id="pane"></div>
        <button aria-label="Close dialog"></button></div>`;
      layer.querySelector('[aria-label="Close dialog"]')!.addEventListener("click", () => {
        layer.innerHTML = "";
      });
      const pane = layer.querySelector("#pane")!;
      [...layer.querySelectorAll('[role="tab"]')]
        .find((tab) => tab.textContent === "Video")!
        .addEventListener("click", () => {
          pane.innerHTML = `<div role="combobox" aria-label="Camera: ${current}">${current}</div>
            <div id="list"></div>`;
          const picker = pane.querySelector('[role="combobox"]')!;
          const list = pane.querySelector("#list")!;
          picker.addEventListener("click", () => {
            if (list.childElementCount) return void (list.innerHTML = "");
            list.innerHTML = cameras
              .map((name) => `<div role="option" aria-selected="${name === current}">${name}</div>`)
              .join("");
            for (const option of list.querySelectorAll('[role="option"]'))
              option.addEventListener("click", () => {
                current = option.textContent!;
                picks.push(current);
                picker.setAttribute("aria-label", `Camera: ${current}`);
                picker.textContent = current;
                list.innerHTML = "";
              });
          });
        });
    });
  });
  return { picks, layer, current: () => current };
}

function install(): MeetingCamera {
  const context = {
    document,
    location: window.location,
    sessionStorage: window.sessionStorage,
    KeyboardEvent: window.KeyboardEvent,
    setTimeout,
    Date,
    globalThis: undefined as unknown,
  };
  context.globalThis = context;
  vm.runInNewContext(source, context);
  return (context as unknown as { anarlogMeetingCamera: MeetingCamera }).anarlogMeetingCamera;
}

async function select(camera: MeetingCamera, name: string) {
  const result = camera.select(name);
  // Past the ten minutes it waits for the person to join.
  await vi.advanceTimersByTimeAsync(11 * 60_000);
  return result;
}

beforeEach(() => {
  vi.useFakeTimers();
  window.sessionStorage.clear();
});
afterEach(() => vi.useRealTimers());

it("picks Anarlog Camera in Meet's settings and closes them again", async () => {
  const page = meetPage();
  expect(await select(install(), "Anarlog Camera")).toBe("switched");
  expect(page.current()).toBe("Anarlog Camera");
  expect(page.layer.innerHTML).toBe("");
});

it("leaves a call already using Anarlog Camera alone", async () => {
  const page = meetPage({ current: "Anarlog Camera" });
  expect(await select(install(), "Anarlog Camera")).toBe("already_using");
  expect(page.picks).toEqual([]);
  expect(page.layer.innerHTML).toBe("");
});

it("keeps the person's camera when Meet doesn't list Anarlog Camera", async () => {
  const page = meetPage({ cameras: ["MacBook Pro Camera"] });
  expect(await select(install(), "Anarlog Camera")).toBe("camera_missing");
  expect(page.current()).toBe("MacBook Pro Camera");
  expect(page.layer.innerHTML).toBe("");
});

it("never matches a camera by part of its name", async () => {
  const page = meetPage({ cameras: ["MacBook Pro Camera", "Anarlog Camera (dev)"] });
  expect(await select(install(), "Anarlog Camera")).toBe("camera_missing");
  expect(page.picks).toEqual([]);
});

it("gives up before the person joins, touching nothing", async () => {
  const page = meetPage({ inCall: false });
  expect(await select(install(), "Anarlog Camera")).toBe("not_in_call");
  expect(page.layer.innerHTML).toBe("");
});

it("does nothing once the recording stops before it acts", async () => {
  const page = meetPage();
  const camera = install();
  const result = camera.select("Anarlog Camera");
  camera.cancel();
  await vi.advanceTimersByTimeAsync(11 * 60_000);
  expect(await result).toBe("cancelled");
  expect(page.picks).toEqual([]);
  expect(page.layer.innerHTML).toBe("");
});

it("tries once per call, so a later recording keeps the camera the person picked", async () => {
  const page = meetPage();
  expect(await select(install(), "Anarlog Camera")).toBe("switched");
  // The person switches back by hand, then records again in the same call.
  page.layer.innerHTML = "";
  expect(await select(install(), "Anarlog Camera")).toBe("already_tried");
  expect(page.picks).toEqual(["Anarlog Camera"]);
});
