import { createRequire } from "node:module";
import type {
  ReactScanDevtools as ScanDevtools,
  ScanNotification,
} from "react-scan";
import { expect, it } from "vitest";

// The CJS entry avoids Node's JSON import-attribute limitation in Scan's ESM
// entry. This exercises our installed patch and the actual upstream templates.
const { ReactScanDevtools } = createRequire(import.meta.url)("react-scan") as {
  ReactScanDevtools: typeof ScanDevtools;
};
const components: ScanNotification["groupedFiberRenders"] = [
  {
    id: "card-render",
    name: "SlowCard",
    count: 7,
    totalTime: 42,
    hasMemoCache: false,
    wasFiberRenderMount: false,
    changes: {
      props: [{ name: "expanded", count: 3 }],
      context: [],
      state: [{ index: 1, count: 2 }],
    },
  },
];
const events: ScanNotification[] = [
  {
    id: "click",
    kind: "interaction",
    type: "click",
    componentPath: ["Page", "SlowCard"],
    timestamp: 100,
    groupedFiberRenders: components,
    timing: {
      kind: "interaction",
      renderTime: 42,
      otherJSTime: 120,
      framePreparation: 10,
      frameConstruction: 50,
      frameDraw: 20,
    },
  },
  {
    id: "frame",
    kind: "dropped-frames",
    fps: 20,
    timestamp: 200,
    groupedFiberRenders: components,
    timing: { kind: "dropped-frames", renderTime: 42, otherTime: 210 },
  },
];

for (const event of events) {
  it(`generates all three upstream prompts from real ${event.kind} data`, () => {
    const prompts = ["fix", "explanation", "data"].map((mode) =>
      ReactScanDevtools.getPrompt(
        mode as "fix" | "explanation" | "data",
        event,
      ),
    );
    expect(new Set(prompts).size).toBe(3);
    for (const prompt of prompts) {
      expect(prompt).toContain("SlowCard");
      expect(prompt).toContain("expanded");
      expect(prompt).toContain("42");
      expect(prompt).not.toContain("undefined");
      expect(prompt.length).toBeGreaterThan(200);
    }
  });
}
it("exposes the bounded event store and the full inspector without a floating toolbar", () => {
  expect(ReactScanDevtools.getEvents()).toEqual([]);
  let notifications = 0;
  const unsubscribe = ReactScanDevtools.subscribe(() => notifications++);
  ReactScanDevtools.clear();
  expect(notifications).toBe(1);
  unsubscribe();
  ReactScanDevtools.clear();
  expect(notifications).toBe(1);
  expect(typeof ReactScanDevtools.mountInspector).toBe("function");
  expect(document.querySelector("#react-scan-toolbar-root")).toBeNull();
});
