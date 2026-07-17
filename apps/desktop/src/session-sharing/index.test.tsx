import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  auth: {
    session: null as any,
    supabase: {} as any,
    signIn: vi.fn().mockResolvedValue(undefined),
  },
  billing: {
    isReady: true,
    isPaid: true,
    upgradeToPro: vi.fn(),
  },
  events: [] as string[],
  access: [] as any[],
  management: null as any,
  loadSessionShareSource: vi.fn(),
  createOrReuseSessionShare: vi.fn(),
  publishSessionShareSnapshot: vi.fn(),
  getSessionShareManagement: vi.fn(),
  listSessionShareAccess: vi.fn(),
  enableSessionShareLink: vi.fn(),
  rotateSessionShareLink: vi.fn(),
  createSessionAccessInvitation: vi.fn(),
  resendSessionAccessInvitation: vi.fn(),
  revokeSessionAccessInvitation: vi.fn(),
  updateSessionAccessGrant: vi.fn(),
  revokeSessionAccessGrant: vi.fn(),
  reviewSessionAccessRequest: vi.fn(),
  setSessionShareScope: vi.fn(),
  upsertDurableSharedNoteCache: vi.fn().mockResolvedValue(undefined),
  durableNote: null as any,
  sessionAttachments: [] as any[],
  sharedAttachmentMap: new Map<string, string>(),
  attachmentControlProps: null as any,
  attachmentMetadataMatches: vi.fn(() => true),
  isAttachmentShareable: vi.fn(
    (attachment: any) => attachment.localAvailability === "present",
  ),
  loadSessionShareAttachments: vi.fn(),
  prepareSessionShareAttachment: vi.fn(),
  setAttachmentCloudSyncEnabled: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
  clipboardWriteText: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("~/auth", () => ({
  useAuth: () => mocks.auth,
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("~/shared-notes/cache", () => ({
  upsertDurableSharedNoteCache: mocks.upsertDurableSharedNoteCache,
  useDurableSharedNote: () => ({
    data: mocks.durableNote,
    isLoading: false,
  }),
}));

vi.mock("./attachments", () => ({
  addSharedAttachmentIds: (body: unknown) => body,
  attachmentMetadataMatches: mocks.attachmentMetadataMatches,
  isAttachmentShareable: mocks.isAttachmentShareable,
  loadSessionShareAttachments: mocks.loadSessionShareAttachments,
  matchSharedAttachmentsToLocal: () => new Map(mocks.sharedAttachmentMap),
  prepareSessionShareAttachment: mocks.prepareSessionShareAttachment,
  useSessionShareAttachments: () => ({ data: mocks.sessionAttachments }),
}));

vi.mock("./attachment-controls", () => ({
  SessionAttachmentControls: (props: unknown) => {
    mocks.attachmentControlProps = props;
    return null;
  },
}));

vi.mock("~/session/attachments", () => ({
  setAttachmentCloudSyncEnabled: mocks.setAttachmentCloudSyncEnabled,
}));

vi.mock("./source", () => ({
  loadSessionShareSource: mocks.loadSessionShareSource,
  useAvailableShareWorkspaces: () => [],
}));

vi.mock("./client", async (importOriginal) => {
  const original = await importOriginal<typeof import("./client")>();
  return {
    ...original,
    createOrReuseSessionShare: mocks.createOrReuseSessionShare,
    createSessionAccessInvitation: mocks.createSessionAccessInvitation,
    enableSessionShareLink: mocks.enableSessionShareLink,
    getSessionShareManagement: mocks.getSessionShareManagement,
    listSessionShareAccess: mocks.listSessionShareAccess,
    publishSessionShareSnapshot: mocks.publishSessionShareSnapshot,
    resendSessionAccessInvitation: mocks.resendSessionAccessInvitation,
    reviewSessionAccessRequest: mocks.reviewSessionAccessRequest,
    revokeSessionAccessGrant: mocks.revokeSessionAccessGrant,
    revokeSessionAccessInvitation: mocks.revokeSessionAccessInvitation,
    rotateSessionShareLink: mocks.rotateSessionShareLink,
    setSessionShareScope: mocks.setSessionShareScope,
    updateSessionAccessGrant: mocks.updateSessionAccessGrant,
  };
});

vi.mock("@hypr/ui/components/ui/dialog", () => ({
  Dialog: ({ open, children }: { open: boolean; children: React.ReactNode }) =>
    open ? <div role="dialog">{children}</div> : null,
  DialogContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DialogDescription: ({ children }: { children: React.ReactNode }) => (
    <p>{children}</p>
  ),
  DialogFooter: ({ children }: { children: React.ReactNode }) => (
    <footer>{children}</footer>
  ),
  DialogHeader: ({ children }: { children: React.ReactNode }) => (
    <header>{children}</header>
  ),
  DialogTitle: ({ children }: { children: React.ReactNode }) => (
    <h2>{children}</h2>
  ),
}));

vi.mock("@hypr/ui/components/ui/select", () => ({
  Select: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SelectContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  SelectItem: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  SelectTrigger: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  SelectValue: () => null,
}));

vi.mock("@hypr/ui/components/ui/toast", () => ({
  sonnerToast: {
    success: mocks.toastSuccess,
    error: mocks.toastError,
  },
}));

import { SessionShareButton } from "./index";

const USER_ID = "11111111-1111-4111-8111-111111111111";
const OTHER_USER_ID = "77777777-7777-4777-8777-777777777777";
const WORKSPACE_ID = "22222222-2222-4222-8222-222222222222";
const SHARE_ID = "33333333-3333-4333-8333-333333333333";
const LINK_ID = "44444444-4444-4444-8444-444444444444";
const INVITATION_ID = "55555555-5555-4555-8555-555555555555";
const GRANT_ID = "66666666-6666-4666-8666-666666666666";
const REQUEST_ID = "88888888-8888-4888-8888-888888888888";
const TOKEN = "t".repeat(43);
const PUBLIC_SLUG = `s_${"a".repeat(32)}`;

function createSession(userId = USER_ID) {
  return {
    access_token: `access-token-${userId}`,
    token_type: "bearer",
    user: { id: userId, is_anonymous: false },
  };
}

function defaultManagement(overrides: Record<string, unknown> = {}) {
  return {
    shareId: SHARE_ID,
    workspaceId: WORKSPACE_ID,
    sessionId: "session-1",
    generalScope: "restricted",
    generalWorkspaceId: null,
    publicSlug: PUBLIC_SLUG,
    hasActiveLink: false,
    accessVersion: 1,
    ...overrides,
  };
}

function renderShareButton() {
  return renderShareButtonView().queryClient;
}

function renderShareButtonView() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const element = () => (
    <QueryClientProvider client={queryClient}>
      <SessionShareButton sessionId="session-1" />
    </QueryClientProvider>
  );
  const view = render(element());
  return {
    queryClient,
    rerender: () => view.rerender(element()),
  };
}

async function openShareDialog() {
  fireEvent.click(screen.getByRole("button", { name: "Share note" }));
  await screen.findByRole("heading", { name: "Share note" });
}

describe("SessionShareButton", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.events = [];
    mocks.access = [];
    mocks.auth.session = createSession();
    mocks.auth.supabase = {};
    mocks.billing.isReady = true;
    mocks.billing.isPaid = true;
    mocks.management = defaultManagement();
    mocks.sessionAttachments = [];
    mocks.sharedAttachmentMap = new Map();
    mocks.attachmentControlProps = null;
    mocks.loadSessionShareAttachments.mockResolvedValue([]);
    mocks.setAttachmentCloudSyncEnabled.mockImplementation(
      async (_sessionId: string, _attachmentId: string, enabled: boolean) => {
        mocks.events.push(enabled ? "cloud-on" : "cloud-off");
      },
    );
    mocks.durableNote = {
      shareId: SHARE_ID,
      workspaceId: WORKSPACE_ID,
      sessionId: "session-1",
      schemaVersion: 1,
      contentRevision: 1,
      title: "Planning",
      body: { type: "doc", content: [] },
      attachments: [],
      capability: "editor",
      manageAccess: true,
      accessVersion: 1,
      publishedAt: "2026-07-17T00:00:00Z",
    };
    mocks.loadSessionShareSource.mockImplementation(async () => {
      mocks.events.push("load");
      return {
        sessionId: "session-1",
        workspaceId: WORKSPACE_ID,
        title: "Planning",
        body: { type: "doc", content: [] },
      };
    });
    mocks.createOrReuseSessionShare.mockImplementation(async () => {
      mocks.events.push("create");
      return {
        shareId: SHARE_ID,
        generalScope: "restricted",
        publicSlug: PUBLIC_SLUG,
        accessVersion: 1,
        wasCreated: true,
      };
    });
    mocks.publishSessionShareSnapshot.mockImplementation(async () => {
      mocks.events.push("publish");
      return {
        shareId: SHARE_ID,
        schemaVersion: 1,
        contentRevision: 1,
        title: "Planning",
        body: { type: "doc", content: [] },
        attachments: [],
        publishedAt: "2026-07-17T00:00:00Z",
      };
    });
    mocks.getSessionShareManagement.mockImplementation(async () => {
      mocks.events.push("management");
      return mocks.management;
    });
    mocks.listSessionShareAccess.mockImplementation(async () => {
      mocks.events.push("access");
      return mocks.access;
    });
    mocks.rotateSessionShareLink.mockImplementation(async () => {
      mocks.events.push("rotate-link");
      return {
        shareId: SHARE_ID,
        linkId: LINK_ID,
        linkToken: TOKEN,
        accessVersion: 2,
        wasCreated: true,
      };
    });
    mocks.createSessionAccessInvitation.mockImplementation(async () => {
      mocks.events.push("create-invitation");
      return {
        invitationId: INVITATION_ID,
        inviteToken: TOKEN,
        invitationExpiresAt: "2026-08-17T00:00:00Z",
        wasCreated: true,
      };
    });
    mocks.revokeSessionAccessGrant.mockImplementation(async () => {
      mocks.events.push("revoke-grant");
      return {
        grantId: GRANT_ID,
        revokedAt: "2026-07-17T00:00:00Z",
        accessVersion: 2,
      };
    });
    mocks.setSessionShareScope.mockImplementation(async () => {
      mocks.events.push("set-scope");
      return {
        shareId: SHARE_ID,
        generalScope: "restricted",
        generalWorkspaceId: null,
        publicSlug: PUBLIC_SLUG,
        accessVersion: 2,
      };
    });
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: mocks.clipboardWriteText },
    });
  });

  afterEach(() => {
    cleanup();
  });

  it("starts sign-in before attempting to share for a signed-out user", () => {
    mocks.auth.session = null;
    renderShareButton();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));

    expect(mocks.auth.signIn).toHaveBeenCalledOnce();
    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();
  });

  it("opens the Pro upgrade when a free user has no existing share", async () => {
    mocks.billing.isPaid = false;
    mocks.createOrReuseSessionShare.mockRejectedValueOnce(
      new Error("subscription required"),
    );
    renderShareButton();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));

    await waitFor(() =>
      expect(mocks.billing.upgradeToPro).toHaveBeenCalledOnce(),
    );
    expect(mocks.loadSessionShareSource).toHaveBeenCalledOnce();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
  });

  it("validates the remote identity before publishing and then loads access", async () => {
    renderShareButton();

    await openShareDialog();

    expect(mocks.events.slice(0, 5)).toEqual([
      "load",
      "create",
      "management",
      "publish",
      "access",
    ]);
    expect(mocks.upsertDurableSharedNoteCache).toHaveBeenCalledWith(
      USER_ID,
      expect.objectContaining({
        shareId: SHARE_ID,
        workspaceId: WORKSPACE_ID,
        sessionId: "session-1",
        manageAccess: true,
      }),
    );
    expect(
      screen.getByText(
        "Attachments stay private unless you explicitly include them in this shared note.",
      ),
    ).not.toBeNull();
  });

  it("does not clear attachment selections when reopening an existing share", async () => {
    mocks.durableNote.attachments = [
      {
        id: "88888888-8888-4888-8888-888888888888",
        filename: "diagram.png",
        contentType: "image/png",
        sizeBytes: 42,
        sha256: "a".repeat(64),
      },
    ];
    mocks.createOrReuseSessionShare.mockImplementationOnce(async () => {
      mocks.events.push("create");
      return {
        shareId: SHARE_ID,
        generalScope: "restricted",
        publicSlug: PUBLIC_SLUG,
        accessVersion: 1,
        wasCreated: false,
      };
    });
    renderShareButton();

    await openShareDialog();

    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(mocks.events.slice(0, 4)).toEqual([
      "load",
      "create",
      "management",
      "access",
    ]);
  });

  it("prunes a shared attachment whose local version was replaced", async () => {
    const remoteAttachment = {
      id: "88888888-8888-4888-8888-888888888888",
      filename: "diagram.png",
      contentType: "image/png",
      sizeBytes: 42,
      sha256: "a".repeat(64),
    };
    mocks.durableNote.attachments = [remoteAttachment];
    mocks.createOrReuseSessionShare.mockResolvedValueOnce({
      shareId: SHARE_ID,
      generalScope: "restricted",
      publicSlug: PUBLIC_SLUG,
      accessVersion: 1,
      wasCreated: false,
    });
    renderShareButton();
    await openShareDialog();
    mocks.publishSessionShareSnapshot.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Update shared copy" }));

    await waitFor(() =>
      expect(mocks.publishSessionShareSnapshot).toHaveBeenCalledWith(
        expect.objectContaining({ attachmentIds: [] }),
      ),
    );
  });

  it("does not publish a shared attachment after its local source becomes unavailable", async () => {
    const localAttachment = {
      id: "local-attachment",
      filename: "diagram.png",
      contentType: "image/png",
      sizeBytes: 42,
      sha256: "a".repeat(64),
      sourceType: "note_upload",
      sourceId: "diagram.png",
      cloudSyncEnabled: true,
      cloudObjectKey: "private/object.anb1",
      localAvailability: "present",
      transferDirection: null,
      transferPhase: "completed",
      transferError: "",
    };
    mocks.sessionAttachments = [localAttachment];
    mocks.loadSessionShareAttachments.mockResolvedValueOnce([
      { ...localAttachment, localAvailability: "absent" },
    ]);
    mocks.prepareSessionShareAttachment.mockResolvedValueOnce({
      id: "88888888-8888-4888-8888-888888888888",
      filename: localAttachment.filename,
      contentType: localAttachment.contentType,
      sizeBytes: localAttachment.sizeBytes,
      sha256: localAttachment.sha256,
    });
    mocks.createOrReuseSessionShare.mockResolvedValueOnce({
      shareId: SHARE_ID,
      generalScope: "restricted",
      publicSlug: PUBLIC_SLUG,
      accessVersion: 1,
      wasCreated: false,
    });
    renderShareButton();
    await openShareDialog();

    act(() => {
      mocks.attachmentControlProps.onShareChange(localAttachment, true);
    });

    await waitFor(() =>
      expect(mocks.toastError).toHaveBeenCalledWith(
        "Could not update attachment sharing.",
      ),
    );
    expect(mocks.prepareSessionShareAttachment).toHaveBeenCalledWith(
      expect.objectContaining({
        shareId: SHARE_ID,
        attachment: localAttachment,
        signal: expect.any(AbortSignal),
      }),
    );
    expect(mocks.loadSessionShareAttachments).toHaveBeenCalledWith("session-1");
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(mocks.toastSuccess).not.toHaveBeenCalled();
  });

  it("revokes a shared copy before making its private backup local-only", async () => {
    const localAttachment = {
      id: "local-attachment",
      filename: "diagram.png",
      contentType: "image/png",
      sizeBytes: 42,
      sha256: "a".repeat(64),
      sourceType: "note_upload",
      sourceId: "diagram.png",
      cloudSyncEnabled: true,
      cloudObjectKey: "private/object.anb1",
      localAvailability: "present",
      transferDirection: null,
      transferPhase: "completed",
      transferError: "",
    };
    const remoteAttachment = {
      id: "88888888-8888-4888-8888-888888888888",
      filename: localAttachment.filename,
      contentType: localAttachment.contentType,
      sizeBytes: localAttachment.sizeBytes,
      sha256: localAttachment.sha256,
    };
    mocks.sessionAttachments = [localAttachment];
    mocks.loadSessionShareAttachments.mockResolvedValue([localAttachment]);
    mocks.sharedAttachmentMap = new Map([
      [localAttachment.id, remoteAttachment.id],
    ]);
    mocks.durableNote.attachments = [remoteAttachment];
    mocks.createOrReuseSessionShare.mockResolvedValueOnce({
      shareId: SHARE_ID,
      generalScope: "restricted",
      publicSlug: PUBLIC_SLUG,
      accessVersion: 1,
      wasCreated: false,
    });
    renderShareButton();
    await openShareDialog();
    mocks.events = [];

    act(() => {
      mocks.attachmentControlProps.onCloudChange(localAttachment, false);
    });

    await waitFor(() =>
      expect(mocks.setAttachmentCloudSyncEnabled).toHaveBeenCalledWith(
        "session-1",
        localAttachment.id,
        false,
      ),
    );
    expect(mocks.publishSessionShareSnapshot).toHaveBeenCalledWith(
      expect.objectContaining({ attachmentIds: [] }),
    );
    expect(mocks.events.slice(0, 3)).toEqual(["load", "publish", "cloud-off"]);
  });

  it("abandons initial share preparation when the active account changes", async () => {
    let resolveSource!: (source: {
      sessionId: string;
      workspaceId: string;
      title: string;
      body: { type: "doc"; content: never[] };
    }) => void;
    mocks.loadSessionShareSource.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveSource = resolve;
      }),
    );
    const view = renderShareButtonView();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    await waitFor(() =>
      expect(mocks.loadSessionShareSource).toHaveBeenCalledOnce(),
    );

    mocks.auth.session = createSession(OTHER_USER_ID);
    view.rerender();
    await act(async () => {
      resolveSource({
        sessionId: "session-1",
        workspaceId: WORKSPACE_ID,
        title: "Planning",
        body: { type: "doc", content: [] },
      });
    });

    await waitFor(() =>
      expect(
        (
          screen.getByRole("button", {
            name: "Share note",
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(false),
    );
    expect(mocks.createOrReuseSessionShare).not.toHaveBeenCalled();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("publishes before rotating a bearer link and keeps the token out of query keys", async () => {
    mocks.management = defaultManagement({
      generalScope: "link",
      hasActiveLink: true,
    });
    const queryClient = renderShareButton();
    await openShareDialog();
    mocks.events = [];
    mocks.clipboardWriteText.mockClear();

    fireEvent.click(
      screen.getByRole("button", { name: "Replace link & copy" }),
    );

    await waitFor(() =>
      expect(mocks.clipboardWriteText).toHaveBeenCalledOnce(),
    );
    expect(mocks.events.slice(0, 3)).toEqual([
      "load",
      "publish",
      "rotate-link",
    ]);
    const copied = new URL(mocks.clipboardWriteText.mock.calls[0]![0]);
    expect(copied.search).toBe("");
    expect(copied.hash).toBe(`#token=${TOKEN}`);
    expect(
      JSON.stringify(
        queryClient
          .getQueryCache()
          .getAll()
          .map((query) => query.queryKey),
      ),
    ).not.toContain(TOKEN);
  });

  it("publishes before creating an invitation and copies its fragment URL", async () => {
    renderShareButton();
    await openShareDialog();
    mocks.events = [];
    mocks.clipboardWriteText.mockClear();

    fireEvent.change(screen.getByRole("textbox", { name: "Invitee email" }), {
      target: { value: "person@example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Copy invite" }));

    await waitFor(() =>
      expect(mocks.clipboardWriteText).toHaveBeenCalledOnce(),
    );
    expect(mocks.events.slice(0, 3)).toEqual([
      "load",
      "publish",
      "create-invitation",
    ]);
    const copied = new URL(mocks.clipboardWriteText.mock.calls[0]![0]);
    expect(copied.pathname).toBe(`/share/invite/${INVITATION_ID}/`);
    expect(copied.search).toBe("");
    expect(copied.hash).toBe(`#token=${TOKEN}`);
  });

  it("restricts a newly rotated link when copying the bearer token fails", async () => {
    mocks.management = defaultManagement({
      generalScope: "link",
      hasActiveLink: true,
    });
    renderShareButton();
    await openShareDialog();
    mocks.clipboardWriteText.mockRejectedValueOnce(new Error("clipboard"));
    mocks.setSessionShareScope.mockClear();

    fireEvent.click(
      screen.getByRole("button", { name: "Replace link & copy" }),
    );

    await waitFor(() =>
      expect(mocks.setSessionShareScope).toHaveBeenCalledWith(
        expect.anything(),
        { shareId: SHARE_ID, scope: "restricted" },
      ),
    );
    expect(mocks.toastError).toHaveBeenCalledWith(
      "Could not update general access.",
    );
  });

  it("restricts a committed link rotation when the active account changes before copy", async () => {
    mocks.management = defaultManagement({
      generalScope: "link",
      hasActiveLink: true,
    });
    let resolveRotation!: (link: {
      shareId: string;
      linkId: string;
      linkToken: string;
      accessVersion: number;
      wasCreated: boolean;
    }) => void;
    mocks.rotateSessionShareLink.mockImplementationOnce(async () => {
      mocks.events.push("rotate-link");
      return await new Promise((resolve) => {
        resolveRotation = resolve;
      });
    });
    const view = renderShareButtonView();
    await openShareDialog();
    mocks.clipboardWriteText.mockClear();
    mocks.setSessionShareScope.mockClear();

    fireEvent.click(
      screen.getByRole("button", { name: "Replace link & copy" }),
    );
    await waitFor(() =>
      expect(mocks.rotateSessionShareLink).toHaveBeenCalledOnce(),
    );

    mocks.auth.session = createSession(OTHER_USER_ID);
    view.rerender();
    await act(async () => {
      resolveRotation({
        shareId: SHARE_ID,
        linkId: LINK_ID,
        linkToken: TOKEN,
        accessVersion: 2,
        wasCreated: true,
      });
    });

    await waitFor(() =>
      expect(mocks.setSessionShareScope).toHaveBeenCalledWith(
        expect.anything(),
        { shareId: SHARE_ID, scope: "restricted" },
      ),
    );
    expect(mocks.clipboardWriteText).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("revokes a grant even when no new snapshot is published", async () => {
    mocks.access = [
      {
        entryType: "grant",
        entryId: GRANT_ID,
        userId: "77777777-7777-4777-8777-777777777777",
        userEmail: "person@example.com",
        capability: "viewer",
        status: "active",
        createdAt: "2026-07-17T00:00:00Z",
        expiresAt: null,
      },
    ];
    renderShareButton();
    await openShareDialog();
    mocks.events = [];
    mocks.publishSessionShareSnapshot.mockClear();

    fireEvent.click(
      screen.getByRole("button", {
        name: "Remove access for person@example.com",
      }),
    );

    await waitFor(() =>
      expect(mocks.revokeSessionAccessGrant).toHaveBeenCalledOnce(),
    );
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(mocks.events[0]).toBe("revoke-grant");
  });

  it("publishes before approving a pending access request", async () => {
    mocks.access = [
      {
        entryType: "request",
        entryId: REQUEST_ID,
        userId: OTHER_USER_ID,
        userEmail: "requester@example.com",
        capability: "commenter",
        status: "pending",
        createdAt: "2026-07-17T00:00:00Z",
        expiresAt: null,
      },
    ];
    mocks.reviewSessionAccessRequest.mockImplementation(
      async (_context: unknown, input: { decision: "approve" | "deny" }) => {
        mocks.events.push(`${input.decision}-request`);
      },
    );
    renderShareButton();
    await openShareDialog();
    expect(screen.getByText("Requested can comment")).not.toBeNull();
    mocks.events = [];
    mocks.publishSessionShareSnapshot.mockClear();
    mocks.reviewSessionAccessRequest.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Approve" }));

    await waitFor(() =>
      expect(mocks.reviewSessionAccessRequest).toHaveBeenCalledWith(
        expect.anything(),
        {
          requestId: REQUEST_ID,
          decision: "approve",
          capability: "commenter",
        },
      ),
    );
    expect(mocks.publishSessionShareSnapshot).toHaveBeenCalledOnce();
    expect(mocks.events.indexOf("publish")).toBeGreaterThanOrEqual(0);
    expect(mocks.events.indexOf("approve-request")).toBeGreaterThan(
      mocks.events.indexOf("publish"),
    );
  });

  it("denies a pending access request without publishing", async () => {
    mocks.access = [
      {
        entryType: "request",
        entryId: REQUEST_ID,
        userId: OTHER_USER_ID,
        userEmail: "requester@example.com",
        capability: "editor",
        status: "pending",
        createdAt: "2026-07-17T00:00:00Z",
        expiresAt: null,
      },
    ];
    mocks.reviewSessionAccessRequest.mockImplementation(
      async (_context: unknown, input: { decision: "approve" | "deny" }) => {
        mocks.events.push(`${input.decision}-request`);
      },
    );
    renderShareButton();
    await openShareDialog();
    expect(screen.getByText("Requested can edit")).not.toBeNull();
    mocks.events = [];
    mocks.publishSessionShareSnapshot.mockClear();
    mocks.reviewSessionAccessRequest.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Deny" }));

    await waitFor(() =>
      expect(mocks.reviewSessionAccessRequest).toHaveBeenCalledWith(
        expect.anything(),
        { requestId: REQUEST_ID, decision: "deny" },
      ),
    );
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(mocks.events[0]).toBe("deny-request");
  });

  it("lets an expired Pro user reopen an existing share to revoke access", async () => {
    mocks.billing.isPaid = false;
    mocks.access = [
      {
        entryType: "grant",
        entryId: GRANT_ID,
        userId: "77777777-7777-4777-8777-777777777777",
        userEmail: "person@example.com",
        capability: "editor",
        status: "active",
        createdAt: "2026-07-17T00:00:00Z",
        expiresAt: null,
      },
    ];
    renderShareButton();

    await openShareDialog();

    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(
      screen.getByText(
        "Upgrade to expand access. You can still restrict or revoke existing access.",
      ),
    ).not.toBeNull();

    fireEvent.click(
      screen.getByRole("button", {
        name: "Remove access for person@example.com",
      }),
    );

    await waitFor(() =>
      expect(mocks.revokeSessionAccessGrant).toHaveBeenCalledOnce(),
    );
  });
});
