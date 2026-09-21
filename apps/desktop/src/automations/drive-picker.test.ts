import { beforeEach, expect, it, vi } from "vitest";

import { completeDrivePicker, pickDriveFolder } from "./drive-picker";

const mocks = vi.hoisted(() => ({
  token: vi.fn(),
  validate: vi.fn(),
  open: vi.fn(),
}));
vi.mock("@anlg/api-client", () => ({
  googleDrivePickerStart: mocks.token,
  googleDrivePickerComplete: mocks.validate,
}));
vi.mock("@anlg/plugin-opener2", () => ({ commands: { openUrl: mocks.open } }));
vi.mock("~/shared/utils", () => ({
  buildWebAppUrl: async () =>
    "https://anarlog.so/app/google-drive-picker?flow=desktop&scheme=anarlog-dev",
}));

beforeEach(() => {
  vi.resetAllMocks();
  mocks.token.mockResolvedValue({
    data: {
      authorization_url:
        "https://accounts.google.com/o/oauth2/v2/auth?state=signed-state",
      state: "signed-state",
    },
  });
  mocks.validate.mockResolvedValue({
    data: { id: "folder", name: "Meeting notes" },
  });
});

it("correlates a single callback and validates the folder using the original connection", async () => {
  mocks.open.mockImplementation(async (input: string) => {
    const url = new URL(input);
    expect(url.searchParams.has("access_token")).toBe(false);
    const hash = new URLSearchParams(url.hash.slice(1));
    expect(hash.has("access_token")).toBe(false);
    expect(hash.get("authorization_url")).toContain("accounts.google.com");
    expect(completeDrivePicker("unrelated")).toBe(false);
    completeDrivePicker("drive-picker:unknown:wrong-folder");
    const callback = `drive-picker:${hash.get("request_id")}:${encodeURIComponent(JSON.stringify({ state: "signed-state", code: "code", folder_id: "folder" }))}`;
    completeDrivePicker(callback);
    completeDrivePicker(callback);
    return { status: "ok", data: null };
  });
  const result = await pickDriveFolder(
    {} as Parameters<typeof pickDriveFolder>[0],
    "connection",
  );
  expect(result?.id).toBe("folder");
  expect(mocks.validate).toHaveBeenCalledExactlyOnceWith({
    client: {},
    body: { state: "signed-state", code: "code", folder_id: "folder" },
  });
});

it("leaves the destination unchanged when selection is cancelled", async () => {
  mocks.open.mockImplementation(async (input: string) => {
    const requestId = new URLSearchParams(new URL(input).hash.slice(1)).get(
      "request_id",
    );
    completeDrivePicker(`drive-picker:${requestId}:`);
    return { status: "ok", data: null };
  });
  expect(
    await pickDriveFolder(
      {} as Parameters<typeof pickDriveFolder>[0],
      "connection",
    ),
  ).toBeNull();
  expect(mocks.validate).not.toHaveBeenCalled();
});

it.each([
  { state: "another-state", code: "code", folder_id: "folder" },
  { state: "signed-state", code: "", folder_id: "folder" },
])(
  "rejects invalid OAuth results before completing the connection: %j",
  async (selection) => {
    mocks.open.mockImplementation(async (input: string) => {
      const requestId = new URLSearchParams(new URL(input).hash.slice(1)).get(
        "request_id",
      );
      completeDrivePicker(
        `drive-picker:${requestId}:${encodeURIComponent(JSON.stringify(selection))}`,
      );
      return { status: "ok", data: null };
    });
    await expect(
      pickDriveFolder(
        {} as Parameters<typeof pickDriveFolder>[0],
        "connection",
      ),
    ).rejects.toThrow();
    expect(mocks.validate).not.toHaveBeenCalled();
  },
);

it("treats Google's access_denied response as cancellation", async () => {
  mocks.open.mockImplementation(async (input: string) => {
    const requestId = new URLSearchParams(new URL(input).hash.slice(1)).get(
      "request_id",
    );
    completeDrivePicker(
      `drive-picker:${requestId}:${encodeURIComponent(JSON.stringify({ state: "signed-state", error: "access_denied" }))}`,
    );
    return { status: "ok", data: null };
  });
  await expect(
    pickDriveFolder({} as Parameters<typeof pickDriveFolder>[0], "connection"),
  ).resolves.toBeNull();
  expect(mocks.validate).not.toHaveBeenCalled();
});

it("does not complete a connection when the browser cannot open", async () => {
  mocks.open.mockResolvedValue({ status: "error", error: "unavailable" });
  await expect(
    pickDriveFolder({} as Parameters<typeof pickDriveFolder>[0], "connection"),
  ).rejects.toThrow("Could not open the folder picker");
  expect(mocks.validate).not.toHaveBeenCalled();
});
