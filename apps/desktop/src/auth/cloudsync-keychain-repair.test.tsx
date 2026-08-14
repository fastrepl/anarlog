import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  applyCloudsyncPreference: vi.fn(),
  repairKeychainAccess: vi.fn(),
  setCredentialBlock: vi.fn(),
  signOut: vi.fn(),
  credentialBlock: "keychain_access" as string | null,
  session: { user: { id: "user-1" } },
}));

vi.mock("./auth-context", () => ({
  useAuth: () => ({ session: mocks.session, signOut: mocks.signOut }),
}));

vi.mock("./cloudsync", () => ({
  applyCloudsyncPreference: mocks.applyCloudsyncPreference,
  getCloudsyncCredentialBlock: () => mocks.credentialBlock,
  subscribeCloudsyncCredentialBlock: () => () => {},
}));

vi.mock("./cloudsync-credentials", () => ({
  setCredentialBlock: mocks.setCredentialBlock,
}));

vi.mock("~/shared/keychain", () => ({
  repairKeychainAccess: mocks.repairKeychainAccess,
}));

vi.mock("~/shared/ui/settings-alert", () => ({
  SettingsAlertToast: ({
    description,
    action,
    lifecycle,
  }: {
    description?: string;
    action?: { label: string; onClick: () => void };
    lifecycle: string;
  }) =>
    description ? (
      <div data-testid="keychain-toast" data-lifecycle={lifecycle}>
        <span>{description}</span>
        {action ? (
          <button type="button" onClick={action.onClick}>
            {action.label}
          </button>
        ) : null}
      </div>
    ) : null,
}));

vi.mock("@lingui/react/macro", () => ({
  useLingui: () => ({
    t: (strings: TemplateStringsArray, ...values: unknown[]) =>
      strings.reduce(
        (message, part, index) =>
          `${message}${part}${index < values.length ? String(values[index]) : ""}`,
        "",
      ),
  }),
}));

import { CloudsyncKeychainRepairToast } from "./cloudsync-keychain-repair";

function renderToast() {
  const queryClient = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <CloudsyncKeychainRepairToast />
    </QueryClientProvider>,
  );
}

describe("CloudsyncKeychainRepairToast", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.credentialBlock = "keychain_access";
    mocks.repairKeychainAccess.mockResolvedValue(undefined);
    mocks.applyCloudsyncPreference.mockResolvedValue("ok");
  });

  afterEach(cleanup);

  it("offers persistent Keychain repair and retries cloud sync", async () => {
    renderToast();

    expect(screen.getByTestId("keychain-toast").dataset.lifecycle).toBe(
      "condition-bound",
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Repair Keychain Access" }),
    );

    await vi.waitFor(() =>
      expect(mocks.repairKeychainAccess).toHaveBeenCalledOnce(),
    );
    await vi.waitFor(() =>
      expect(mocks.applyCloudsyncPreference).toHaveBeenCalledWith(
        mocks.session,
      ),
    );
    expect(mocks.signOut).not.toHaveBeenCalled();
    expect(mocks.setCredentialBlock).toHaveBeenCalledWith(null);
  });

  it("signs out when repair finds an account mismatch", async () => {
    mocks.applyCloudsyncPreference.mockResolvedValue("account_mismatch");
    renderToast();

    fireEvent.click(
      screen.getByRole("button", { name: "Repair Keychain Access" }),
    );

    await vi.waitFor(() => expect(mocks.signOut).toHaveBeenCalledOnce());
  });

  it("stays hidden for unrelated sync failures", () => {
    mocks.credentialBlock = "unavailable";

    renderToast();

    expect(screen.queryByTestId("keychain-toast")).toBeNull();
  });
});
