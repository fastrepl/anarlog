import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  platform: vi.fn(() => "macos"),
  setAppIcon: vi.fn(),
  applyAppIconPreference: vi.fn(),
  appIcon: "default",
  theme: "system",
  appIdentifier: "com.hyprnote.stable" as string | undefined,
  billing: {
    isPro: true,
    isUpgradingToPro: false,
    upgradeToPro: vi.fn(),
  },
  toastWarning: vi.fn(),
}));

vi.mock("@tanstack/react-query", () => ({
  useQuery: () => ({ data: mocks.appIdentifier }),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getIdentifier: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: mocks.platform,
}));

vi.mock("~/settings/queries", () => ({
  useSetSettingValue: () => mocks.setAppIcon,
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { warning: mocks.toastWarning },
}));

vi.mock("~/shared/config", () => ({
  useConfigValue: (key: string) =>
    key === "theme" ? mocks.theme : mocks.appIcon,
}));

vi.mock("~/shared/theme/provider", () => ({
  applyAppIconPreference: mocks.applyAppIconPreference,
}));

import { AppIconSelector } from "./app-icon";

describe("AppIconSelector", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    mocks.platform.mockReturnValue("macos");
    mocks.appIcon = "default";
    mocks.theme = "system";
    mocks.appIdentifier = "com.hyprnote.stable";
    mocks.billing.isPro = true;
    mocks.billing.isUpgradingToPro = false;
    mocks.toastWarning.mockClear();
  });

  const iconOptions = () =>
    within(screen.getByRole("radiogroup", { name: "App icon" })).getAllByRole(
      "radio",
    );

  it("applies and stores the selected icon", () => {
    render(<AppIconSelector />);

    const defaultOption = screen.getByRole("radio", { name: "Default" });
    expect(defaultOption.getAttribute("aria-checked")).toBe("true");
    expect(defaultOption.className).not.toMatch(/\bborder(?:-|\s|$)/);
    expect(
      defaultOption.querySelector("[data-app-icon-stage]")?.className,
    ).toContain("opacity-100");
    expect(
      defaultOption
        .querySelector('source[media="(prefers-color-scheme: dark)"]')
        ?.getAttribute("srcset"),
    ).toBe("/assets/app-icons/stable-dark.png");
    expect(defaultOption.querySelector("img")?.getAttribute("src")).toBe(
      "/assets/app-icons/stable-light.png",
    );
    expect(iconOptions()).toHaveLength(9);
    expect(screen.queryByRole("radio", { name: "Production" })).toBeNull();
    expect(screen.getByRole("radio", { name: "Blueprint" })).toBeDefined();
    expect(screen.getByRole("radio", { name: "Sketch" })).toBeDefined();
    expect(screen.getByRole("radio", { name: "Field Journal" })).toBeDefined();
    expect(screen.getByRole("radio", { name: "Notepad" })).toBeDefined();
    expect(screen.getByRole("radio", { name: "Stone" })).toBeDefined();
    expect(screen.getByRole("radio", { name: "Typewriter Key" })).toBeDefined();
    expect(screen.getByRole("radio", { name: "Walnut" })).toBeDefined();
    expect(screen.queryByText("Blueprint")).toBeNull();

    fireEvent.click(screen.getByRole("radio", { name: "Blueprint" }));

    expect(mocks.applyAppIconPreference).toHaveBeenCalledWith("dev", "system");
    expect(mocks.setAppIcon).toHaveBeenCalledWith("dev");
  });

  it("toasts instead of changing icons on the free plan", () => {
    mocks.billing.isPro = false;

    render(<AppIconSelector />);

    const defaultOption = screen.getByRole("radio", { name: "Default" });
    const blueprintOption = screen.getByRole("radio", { name: "Blueprint" });
    expect(defaultOption.getAttribute("aria-disabled")).toBe("false");
    expect(blueprintOption.getAttribute("aria-disabled")).toBe("true");

    fireEvent.click(blueprintOption);

    expect(mocks.toastWarning).toHaveBeenCalledWith(
      "This requires Anarlog Pro",
      {
        action: {
          label: "Upgrade",
          onClick: expect.any(Function),
        },
      },
    );
    expect(mocks.billing.upgradeToPro).not.toHaveBeenCalled();
    expect(mocks.applyAppIconPreference).not.toHaveBeenCalled();
    expect(mocks.setAppIcon).not.toHaveBeenCalled();
  });

  it("previews both schemes for the system theme", () => {
    render(<AppIconSelector />);

    expect(
      screen
        .getByRole("radio", { name: "Default" })
        .querySelector('source[media="(prefers-color-scheme: dark)"]'),
    ).not.toBeNull();

    const journalOption = screen.getByRole("radio", {
      name: "Field Journal",
    });
    expect(journalOption.querySelector("source")).toBeNull();
    expect(journalOption.querySelector("img")?.getAttribute("src")).toBe(
      "/assets/app-icons/journal.png",
    );
  });

  it("uses the stable icon while the app identifier is loading", () => {
    mocks.appIdentifier = undefined;

    render(<AppIconSelector />);

    expect(screen.queryByRole("radio", { name: "Production" })).toBeNull();
    expect(screen.getByRole("radio", { name: "Blueprint" })).toBeDefined();
  });

  it("pins previews to an explicit theme", () => {
    mocks.theme = "dark";

    render(<AppIconSelector />);

    const defaultOption = screen.getByRole("radio", { name: "Default" });
    expect(
      defaultOption.querySelector(
        'source[media="(prefers-color-scheme: dark)"]',
      ),
    ).toBeNull();
    expect(defaultOption.querySelector("img")?.getAttribute("src")).toBe(
      "/assets/app-icons/stable-dark.png",
    );

    fireEvent.click(screen.getByRole("radio", { name: "Blueprint" }));

    expect(mocks.applyAppIconPreference).toHaveBeenCalledWith("dev", "dark");
  });

  it("previews the current channel icon for the default option", () => {
    mocks.appIdentifier = "com.hyprnote.staging";

    render(<AppIconSelector />);

    const defaultOption = screen.getByRole("radio", { name: "Default" });
    expect(defaultOption.querySelector("img")?.getAttribute("src")).toBe(
      "/assets/app-icons/staging-light.png",
    );
    expect(screen.queryByRole("radio", { name: "Sketch" })).toBeNull();
    expect(screen.getByRole("radio", { name: "Production" })).toBeDefined();
  });

  it("selects default for an equivalent channel-specific preference", () => {
    mocks.appIcon = "stable";

    render(<AppIconSelector />);

    expect(
      screen
        .getByRole("radio", { name: "Default" })
        .getAttribute("aria-checked"),
    ).toBe("true");
    expect(screen.queryByRole("radio", { name: "Production" })).toBeNull();
  });

  it("is hidden on platforms that cannot change the running app icon", () => {
    mocks.platform.mockReturnValue("windows");

    render(<AppIconSelector />);

    expect(screen.queryByText("App icon")).toBeNull();
  });
});
