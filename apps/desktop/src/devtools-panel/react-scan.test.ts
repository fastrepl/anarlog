import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const scan = vi.fn();

vi.mock("react-scan", () => ({
  scan,
}));

import { enableReactScan } from "./react-scan";

describe("enableReactScan", () => {
  beforeEach(() => {
    scan.mockReset();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("does nothing when disabled", async () => {
    await enableReactScan(false);

    expect(scan).not.toHaveBeenCalled();
  });

  it("starts the toolbar when enabled", async () => {
    await enableReactScan(true);

    expect(scan).toHaveBeenCalledWith({
      enabled: true,
      showToolbar: true,
      dangerouslyForceRunInProduction: !import.meta.env.DEV,
    });
  });
});
