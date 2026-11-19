// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import vm from "node:vm";
import { expect, it, vi } from "vitest";

it("stops on an explicit leave action or ended heading, not missing controls or unrelated text", async () => {
  const sendMessage = vi.fn(() => Promise.resolve());
  vm.runInNewContext(
    readFileSync(
      resolve(import.meta.dirname, "../extension/meeting-content.js"),
      "utf8",
    ),
    {
      document,
      MutationObserver,
      setTimeout,
      clearTimeout,
      chrome: { runtime: { sendMessage } },
    },
  );
  document.body.innerHTML = "<h2>Loading</h2><p>You left the meeting</p>";
  await new Promise((resolve) => setTimeout(resolve, 200));
  expect(sendMessage).not.toHaveBeenCalled();
  document.body.innerHTML =
    '<button aria-label="Leave call"><span>Leave</span></button>';
  document.querySelector("span")!.click();
  expect(sendMessage).toHaveBeenCalledWith({ type: "anarlog_meeting_ended" });
  sendMessage.mockClear();
  document.body.innerHTML = "<h1>You left the meeting</h1>";
  await vi.waitFor(() =>
    expect(sendMessage).toHaveBeenCalledWith({ type: "anarlog_meeting_ended" }),
  );
});

it("still notices the call ending while the Meet page keeps changing", async () => {
  const sendMessage = vi.fn(() => Promise.resolve());
  document.body.innerHTML = "";
  vm.runInNewContext(
    readFileSync(
      resolve(import.meta.dirname, "../extension/meeting-content.js"),
      "utf8",
    ),
    {
      document,
      MutationObserver,
      setTimeout,
      clearTimeout,
      chrome: { runtime: { sendMessage } },
    },
  );
  const ticker = document.createElement("span");
  document.body.append(ticker);
  const busy = setInterval(() => (ticker.textContent = String(Date.now())), 10);
  const heading = document.createElement("h1");
  heading.textContent = "You left the meeting";
  document.body.append(heading);
  try {
    await vi.waitFor(() =>
      expect(sendMessage).toHaveBeenCalledWith({
        type: "anarlog_meeting_ended",
      }),
    );
  } finally {
    clearInterval(busy);
  }
});

it("stops watching when the tab lets go, and watches again when it links again", async () => {
  const sendMessage = vi.fn(() => Promise.resolve());
  document.body.innerHTML = "";
  const script = readFileSync(
    resolve(import.meta.dirname, "../extension/meeting-content.js"),
    "utf8",
  );
  const tab = vm.createContext({
    document,
    MutationObserver,
    setTimeout,
    clearTimeout,
    chrome: { runtime: { sendMessage } },
  });
  vm.runInContext(script, tab);
  vm.runInContext(
    "anarlogMeetingMonitor.stop(); anarlogMeetingMonitor.stop()",
    tab,
  );
  document.body.innerHTML =
    '<button aria-label="Leave call"><span>Leave</span></button>';
  document.querySelector("span")!.click();
  document.body.insertAdjacentHTML(
    "beforeend",
    "<h1>You left the meeting</h1>",
  );
  await new Promise((resolve) => setTimeout(resolve, 200));
  expect(sendMessage).not.toHaveBeenCalled();

  vm.runInContext(script, tab);
  await vi.waitFor(() =>
    expect(sendMessage).toHaveBeenCalledWith({ type: "anarlog_meeting_ended" }),
  );
});
