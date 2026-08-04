import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { cloneElement, StrictMode, type ReactElement, type Ref } from "react";
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
  flushCanonicalSessionEditorChanges: vi.fn().mockResolvedValue(undefined),
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
  sendSessionAccessInvitationEmail: vi.fn(),
  revokeSessionAccessInvitation: vi.fn(),
  updateSessionAccessGrant: vi.fn(),
  revokeSessionAccessGrant: vi.fn(),
  reviewSessionAccessRequest: vi.fn(),
  setSessionShareScope: vi.fn(),
  upsertDurableSharedNoteCache: vi.fn().mockResolvedValue(undefined),
  markSessionShareActivated: vi.fn().mockResolvedValue(undefined),
  loadManagedSharedNoteForSession: vi.fn(),
  durableNote: null as any,
  managedNote: null as any,
  managedNoteLoading: false,
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
  loadSessionShareSyncState: vi.fn(),
  syncStatus: "clean" as "clean" | "conflict" | null,
  recordPublishedSessionShareState: vi.fn().mockResolvedValue(undefined),
  openUrl: vi.fn().mockResolvedValue(undefined),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
  clipboardWriteText: vi.fn().mockResolvedValue(undefined),
  contacts: [] as any[],
  workspaces: [] as { id: string; name: string }[],
}));

vi.mock("~/auth", () => ({
  useAuth: () => mocks.auth,
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => mocks.billing,
}));

vi.mock("~/contacts/queries", () => ({
  useHumans: () => mocks.contacts,
}));

vi.mock("~/contacts/shared", () => ({
  ContactFacehash: ({ name }: { name: string }) => <span>{name[0]}</span>,
}));

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: mocks.openUrl },
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
}));

vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: mocks.clipboardWriteText,
}));

vi.mock("~/shared-notes/cache", () => ({
  loadManagedSharedNoteForSession: mocks.loadManagedSharedNoteForSession,
  markSessionShareActivated: mocks.markSessionShareActivated,
  upsertDurableSharedNoteCache: mocks.upsertDurableSharedNoteCache,
  useDurableSharedNote: () => ({
    data: mocks.durableNote,
    isLoading: false,
  }),
  useManagedDurableSharedNote: () => ({
    data: mocks.managedNote,
    error: null,
    isLoading: mocks.managedNoteLoading,
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
  useAvailableShareWorkspaces: () => mocks.workspaces,
}));

vi.mock("./sync-state", () => ({
  useSessionShareSyncStatus: () => mocks.syncStatus,
}));

vi.mock("./editor-activity", () => ({
  flushCanonicalSessionEditorChanges: mocks.flushCanonicalSessionEditorChanges,
}));

vi.mock("./reconciliation", async (importOriginal) => {
  const original = await importOriginal<typeof import("./reconciliation")>();
  return {
    ...original,
    loadSessionShareSyncState: mocks.loadSessionShareSyncState,
    recordPublishedSessionShareState: mocks.recordPublishedSessionShareState,
  };
});

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
    sendSessionAccessInvitationEmail: mocks.sendSessionAccessInvitationEmail,
    reviewSessionAccessRequest: mocks.reviewSessionAccessRequest,
    revokeSessionAccessGrant: mocks.revokeSessionAccessGrant,
    revokeSessionAccessInvitation: mocks.revokeSessionAccessInvitation,
    rotateSessionShareLink: mocks.rotateSessionShareLink,
    setSessionShareScope: mocks.setSessionShareScope,
    updateSessionAccessGrant: mocks.updateSessionAccessGrant,
  };
});

vi.mock("@anlg/ui/components/ui/popover", () => ({
  AppFloatingPanel: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  Popover: ({
    children,
    onOpenChange,
  }: {
    children: React.ReactNode;
    onOpenChange?: (open: boolean) => void;
  }) => (
    <div
      data-testid="share-popover-root"
      onKeyDown={(event) => {
        if (event.key === "Escape") onOpenChange?.(false);
      }}
    >
      {children}
    </div>
  ),
  PopoverContent: ({
    children,
    className,
  }: {
    children: React.ReactNode;
    className?: string;
  }) => (
    <div data-testid="share-popover" className={className}>
      {children}
    </div>
  ),
  PopoverTrigger: ({
    children,
  }: {
    children: ReactElement<{ ref?: Ref<HTMLButtonElement> }>;
  }) => {
    const childRef = children.props.ref;
    return cloneElement(children, {
      ref: (node: HTMLButtonElement | null) => {
        if (typeof childRef === "function") {
          childRef(node);
        } else if (childRef) {
          childRef.current = node;
        }
      },
    });
  },
}));

vi.mock("@anlg/ui/components/ui/select", async () => {
  const React = await import("react");
  const SelectContext = React.createContext({
    disabled: false,
    onValueChange: (_value: string) => {},
  });
  return {
    Select: ({
      children,
      disabled = false,
      onValueChange,
    }: {
      children: React.ReactNode;
      disabled?: boolean;
      onValueChange: (value: string) => void;
    }) => (
      <SelectContext.Provider value={{ disabled, onValueChange }}>
        <div>{children}</div>
      </SelectContext.Provider>
    ),
    SelectContent: ({ children }: { children: React.ReactNode }) => (
      <div>{children}</div>
    ),
    SelectItem: ({
      children,
      disabled = false,
      value,
    }: {
      children: React.ReactNode;
      disabled?: boolean;
      value: string;
    }) => {
      const select = React.useContext(SelectContext);
      return (
        <button
          disabled={disabled || select.disabled}
          onClick={() => select.onValueChange(value)}
        >
          {children}
        </button>
      );
    },
    SelectSeparator: () => <hr />,
    SelectTrigger: ({ children }: { children: React.ReactNode }) => (
      <span>{children}</span>
    ),
    SelectValue: () => null,
  };
});

vi.mock("@anlg/ui/components/ui/toast", () => ({
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

function renderShareButtonView(
  initialSessionId = "session-1",
  strictMode = false,
) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  let currentSessionId = initialSessionId;
  const element = () => {
    const content = (
      <QueryClientProvider client={queryClient}>
        <SessionShareButton sessionId={currentSessionId} />
      </QueryClientProvider>
    );
    return strictMode ? <StrictMode>{content}</StrictMode> : content;
  };
  const view = render(element());
  return {
    queryClient,
    rerender: (sessionId = currentSessionId) => {
      currentSessionId = sessionId;
      view.rerender(element());
    },
  };
}

async function openSharePopover() {
  fireEvent.click(screen.getByRole("button", { name: "Share note" }));
  await screen.findByRole("textbox", { name: "Invitee email" });
  if (mocks.managedNote) {
    await waitFor(() =>
      expect(mocks.getSessionShareManagement).toHaveBeenCalled(),
    );
  }
}

describe("SessionShareButton", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.createOrReuseSessionShare.mockReset();
    mocks.publishSessionShareSnapshot.mockReset();
    mocks.getSessionShareManagement.mockReset();
    mocks.enableSessionShareLink.mockReset();
    mocks.rotateSessionShareLink.mockReset();
    mocks.setSessionShareScope.mockReset();
    mocks.events = [];
    mocks.access = [];
    mocks.contacts = [];
    mocks.workspaces = [];
    mocks.auth.session = createSession();
    mocks.auth.supabase = {};
    mocks.billing.isReady = true;
    mocks.billing.isPaid = true;
    mocks.syncStatus = "clean";
    mocks.management = defaultManagement();
    mocks.loadManagedSharedNoteForSession.mockResolvedValue({
      shareId: SHARE_ID,
      workspaceId: WORKSPACE_ID,
      sessionId: "session-1",
    });
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
      webEditable: true,
      webEditBase: null,
      publishedAt: "2026-07-17T00:00:00Z",
    };
    mocks.managedNote = mocks.durableNote;
    mocks.managedNoteLoading = false;
    mocks.loadSessionShareSyncState.mockResolvedValue({
      viewerUserId: USER_ID,
      shareId: SHARE_ID,
      sessionId: "session-1",
      acknowledgedContentRevision: 1,
      baselineSourceHash: "a".repeat(64),
      status: "clean",
    });
    mocks.loadSessionShareSource.mockImplementation(async (sessionId) => {
      mocks.events.push("load");
      return {
        sessionId,
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
    mocks.enableSessionShareLink.mockImplementation(async () => {
      mocks.events.push("enable-link");
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
    mocks.sendSessionAccessInvitationEmail.mockResolvedValue(undefined);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("starts sign-in before attempting to share for a signed-out user", () => {
    mocks.auth.session = null;
    mocks.billing.isReady = false;
    renderShareButton();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));

    expect(mocks.auth.signIn).toHaveBeenCalledOnce();
    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();
  });

  it("shows an upgrade state before sharing for a free user", async () => {
    mocks.billing.isPaid = false;
    mocks.managedNote = null;
    renderShareButton();

    const trigger = screen.getByRole("button", { name: "Share note" });
    expect((trigger as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(trigger);

    expect(
      await screen.findByRole("heading", { name: "Share notes with others" }),
    ).not.toBeNull();
    expect(
      screen.getByText(
        "Upgrade to Pro to invite people and share this note with them.",
      ),
    ).not.toBeNull();
    expect(mocks.billing.upgradeToPro).not.toHaveBeenCalled();
    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Upgrade to Pro" }));

    expect(mocks.billing.upgradeToPro).toHaveBeenCalledOnce();
  });

  it("closes a free-plan upgrade state when the account changes", async () => {
    mocks.billing.isPaid = false;
    mocks.managedNote = null;
    const view = renderShareButtonView();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    expect(
      await screen.findByRole("heading", { name: "Share notes with others" }),
    ).not.toBeNull();

    mocks.auth.session = createSession(OTHER_USER_ID);
    view.rerender();

    expect(screen.queryByTestId("share-popover")).toBeNull();
  });

  it("opens the complete panel without a loader while billing access loads", async () => {
    mocks.managedNote = null;
    mocks.billing.isReady = false;
    const view = renderShareButtonView("session-1", true);

    const trigger = screen.getByRole("button", { name: "Share note" });
    expect((trigger as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(trigger);

    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    expect(
      screen.getByRole("textbox", { name: "Invitee email" }),
    ).not.toBeNull();
    expect(screen.queryByText("Loading access…")).toBeNull();
    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();

    mocks.billing.isReady = true;
    view.rerender();

    expect(
      (screen.getByRole("button", { name: "Copy link" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false);
    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();
  });

  it("opens sharing as a popover anchored to the toolbar button", async () => {
    mocks.managedNote = null;
    renderShareButton();

    const trigger = screen.getByRole("button", { name: "Share note" });
    expect(trigger.textContent).toBe("");
    expect(trigger.querySelectorAll("svg")).toHaveLength(1);
    expect(trigger.getAttribute("aria-expanded")).toBe("false");

    await openSharePopover();

    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByTestId("share-popover").className).toContain(
      "min-h-[340px]",
    );
    expect(screen.getByTestId("share-popover").className).toContain(
      "max-h-[min(540px,calc(100vh-64px))]",
    );
    expect(screen.getByTestId("share-popover").className).toContain(
      "w-[440px]",
    );

    fireEvent.click(trigger);

    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByTestId("share-popover")).toBeNull();
    expect(screen.queryByText("Loading access…")).toBeNull();
    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();
    expect(mocks.createOrReuseSessionShare).not.toHaveBeenCalled();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(mocks.markSessionShareActivated).not.toHaveBeenCalled();
  });

  it("shows existing share controls while access is still loading", async () => {
    let resolveManagement!: (
      value: ReturnType<typeof defaultManagement>,
    ) => void;
    mocks.getSessionShareManagement.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveManagement = resolve;
      }),
    );
    renderShareButton();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));

    expect(
      screen.getByRole("textbox", { name: "Invitee email" }),
    ).not.toBeNull();
    expect(screen.queryByText("Loading access…")).toBeNull();
    expect(
      (screen.getByRole("button", { name: "Copy link" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);

    await act(async () => {
      resolveManagement(defaultManagement());
    });
    await waitFor(() =>
      expect(
        (
          screen.getByRole("button", {
            name: "Copy link",
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(false),
    );
  });

  it("keeps preparation open when the trigger ref is recomposed", async () => {
    mocks.managedNote = null;
    const view = renderShareButtonView();

    const trigger = screen.getByRole("button", { name: "Share note" });
    fireEvent.click(trigger);
    expect(
      await screen.findByRole("textbox", { name: "Invitee email" }),
    ).not.toBeNull();

    view.rerender();

    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByTestId("share-popover")).not.toBeNull();

    expect(screen.queryByText("Loading access…")).toBeNull();
    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();
  });

  it("cancels preparation immediately when the pending popover is dismissed", async () => {
    mocks.managedNote = null;
    let resolveSource: ((value: any) => void) | undefined;
    mocks.loadSessionShareSource.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveSource = resolve;
      }),
    );
    renderShareButton();

    const trigger = screen.getByRole("button", { name: "Share note" });
    fireEvent.click(trigger);
    fireEvent.click(await screen.findByRole("button", { name: "Copy link" }));
    await waitFor(() =>
      expect(mocks.loadSessionShareSource).toHaveBeenCalledOnce(),
    );
    expect(
      screen
        .getByRole("button", { name: "Copy link" })
        .querySelector(".animate-spin"),
    ).not.toBeNull();
    expect(trigger.querySelector(".animate-spin")).toBeNull();
    expect(screen.queryByText("Loading access…")).toBeNull();

    fireEvent.keyDown(screen.getByTestId("share-popover-root"), {
      key: "Escape",
    });

    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByTestId("share-popover")).toBeNull();

    await act(async () => {
      resolveSource?.({
        sessionId: "session-1",
        workspaceId: WORKSPACE_ID,
        title: "Planning",
        body: { type: "doc", content: [] },
      });
      await Promise.resolve();
    });

    expect(mocks.createOrReuseSessionShare).not.toHaveBeenCalled();
    expect(mocks.toastError).not.toHaveBeenCalled();
    expect(screen.queryByTestId("share-popover")).toBeNull();
  });

  it("reopens a dismissed draft without activating it", async () => {
    mocks.managedNote = null;
    renderShareButton();

    const trigger = screen.getByRole("button", { name: "Share note" });
    fireEvent.click(trigger);
    fireEvent.keyDown(screen.getByTestId("share-popover-root"), {
      key: "Escape",
    });
    fireEvent.click(trigger);
    expect(
      await screen.findByRole("textbox", { name: "Invitee email" }),
    ).not.toBeNull();
    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();
    expect(mocks.markSessionShareActivated).not.toHaveBeenCalled();
  });

  it("activates sharing only after an explicit action", async () => {
    mocks.managedNote = null;
    mocks.loadManagedSharedNoteForSession.mockResolvedValue(null);
    renderShareButton();

    await openSharePopover();
    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();
    expect(mocks.createOrReuseSessionShare).not.toHaveBeenCalled();
    expect(mocks.markSessionShareActivated).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Copy link" }));

    await waitFor(() =>
      expect(mocks.markSessionShareActivated).toHaveBeenCalledWith(
        USER_ID,
        SHARE_ID,
        "session-1",
      ),
    );

    expect(mocks.flushCanonicalSessionEditorChanges).toHaveBeenCalledWith(
      "session-1",
    );
    expect(
      mocks.flushCanonicalSessionEditorChanges.mock.invocationCallOrder[0],
    ).toBeLessThan(mocks.loadSessionShareSource.mock.invocationCallOrder[0]!);
    expect(mocks.events.slice(0, 6)).toEqual([
      "load",
      "create",
      "management",
      "publish",
      "management",
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
      screen.getByRole("textbox", { name: "Invitee email" }),
    ).not.toBeNull();
  });

  it("bootstraps an existing share after its first snapshot publish failed", async () => {
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    mocks.managedNote = null;
    mocks.loadManagedSharedNoteForSession.mockResolvedValue(null);
    mocks.createOrReuseSessionShare
      .mockResolvedValueOnce({
        shareId: SHARE_ID,
        generalScope: "restricted",
        publicSlug: PUBLIC_SLUG,
        accessVersion: 1,
        wasCreated: true,
      })
      .mockResolvedValueOnce({
        shareId: SHARE_ID,
        generalScope: "restricted",
        publicSlug: PUBLIC_SLUG,
        accessVersion: 1,
        wasCreated: false,
      });
    mocks.publishSessionShareSnapshot.mockRejectedValueOnce(
      new Error("connection lost"),
    );
    renderShareButton();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    fireEvent.click(await screen.findByRole("button", { name: "Copy link" }));
    await waitFor(() =>
      expect(mocks.toastError).toHaveBeenCalledWith(
        "Could not copy the share link.",
      ),
    );
    expect(consoleError).toHaveBeenCalledWith(
      "[session-sharing] could not activate share",
      expect.objectContaining({ message: "connection lost" }),
    );
    expect(screen.getByTestId("share-popover")).not.toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Copy link" }));
    await waitFor(() =>
      expect(mocks.publishSessionShareSnapshot).toHaveBeenCalledTimes(2),
    );

    expect(mocks.publishSessionShareSnapshot.mock.calls[0]![0].mutationId).toBe(
      mocks.publishSessionShareSnapshot.mock.calls[1]![0].mutationId,
    );
    expect(mocks.publishSessionShareSnapshot.mock.calls[1]![0]).toMatchObject({
      shareId: SHARE_ID,
      baseRevision: 0,
      attachmentIds: [],
    });
    expect(mocks.upsertDurableSharedNoteCache).toHaveBeenCalledOnce();
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

    await openSharePopover();

    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(mocks.events.slice(0, 2)).toEqual(["management", "access"]);
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
    await openSharePopover();
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
    await openSharePopover();

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

  it("blocks a manual publish before an editable snapshot is reconciled locally", async () => {
    mocks.createOrReuseSessionShare.mockResolvedValueOnce({
      shareId: SHARE_ID,
      generalScope: "restricted",
      publicSlug: PUBLIC_SLUG,
      accessVersion: 1,
      wasCreated: false,
    });
    mocks.loadSessionShareSyncState.mockResolvedValue(null);
    renderShareButton();
    await openSharePopover();
    mocks.publishSessionShareSnapshot.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Update shared copy" }));

    await waitFor(() =>
      expect(mocks.loadSessionShareSyncState).toHaveBeenCalledWith(
        USER_ID,
        SHARE_ID,
        "session-1",
      ),
    );
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(mocks.toastError).toHaveBeenCalledWith(
      "Could not update the shared copy.",
    );
  });

  it("blocks a manual publish for a legacy read-only snapshot without reconciliation state", async () => {
    mocks.durableNote.webEditable = false;
    mocks.createOrReuseSessionShare.mockResolvedValueOnce({
      shareId: SHARE_ID,
      generalScope: "restricted",
      publicSlug: PUBLIC_SLUG,
      accessVersion: 1,
      wasCreated: false,
    });
    mocks.loadSessionShareSyncState.mockResolvedValue(null);
    renderShareButton();
    await openSharePopover();
    mocks.publishSessionShareSnapshot.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Update shared copy" }));

    await waitFor(() =>
      expect(mocks.loadSessionShareSyncState).toHaveBeenCalledWith(
        USER_ID,
        SHARE_ID,
        "session-1",
      ),
    );
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(mocks.toastError).toHaveBeenCalledWith(
      "Could not update the shared copy.",
    );
  });

  it("surfaces a durable conflict and explicitly publishes desktop edits over the web copy", async () => {
    mocks.syncStatus = "conflict";
    mocks.durableNote.contentRevision = 2;
    mocks.durableNote.webEditBase = {
      contentRevision: 1,
      title: "Planning",
      body: { type: "doc", content: [] },
    };
    mocks.loadSessionShareSyncState.mockResolvedValue({
      viewerUserId: USER_ID,
      shareId: SHARE_ID,
      sessionId: "session-1",
      acknowledgedContentRevision: 1,
      baselineSourceHash: "a".repeat(64),
      status: "conflict",
    });
    mocks.createOrReuseSessionShare.mockResolvedValueOnce({
      shareId: SHARE_ID,
      generalScope: "restricted",
      publicSlug: PUBLIC_SLUG,
      accessVersion: 1,
      wasCreated: false,
    });
    mocks.publishSessionShareSnapshot.mockResolvedValueOnce({
      shareId: SHARE_ID,
      schemaVersion: 1,
      contentRevision: 3,
      title: "Planning",
      body: { type: "doc", content: [] },
      attachments: [],
      accessVersion: 1,
      webEditable: true,
      publishedAt: "2026-07-17T00:01:00Z",
    });
    renderShareButton();
    await openSharePopover();

    expect(
      screen.getByRole("heading", {
        name: "Sharing paused to protect your edits",
      }),
    ).not.toBeNull();
    expect(
      (
        screen.getByRole("button", {
          name: "Update shared copy",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Open web copy" }));
    await waitFor(() => expect(mocks.openUrl).toHaveBeenCalledOnce());
    expect(mocks.openUrl).toHaveBeenCalledWith(
      expect.stringContaining(`/share/${SHARE_ID}/`),
      null,
    );

    mocks.flushCanonicalSessionEditorChanges.mockClear();
    mocks.loadSessionShareSource.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Keep desktop edits" }));

    await waitFor(() =>
      expect(mocks.publishSessionShareSnapshot).toHaveBeenCalledWith(
        expect.objectContaining({
          shareId: SHARE_ID,
          baseRevision: 2,
          title: "Planning",
        }),
      ),
    );
    expect(mocks.recordPublishedSessionShareState).toHaveBeenCalledWith(
      expect.objectContaining({
        shareId: SHARE_ID,
        contentRevision: 3,
      }),
    );
    expect(mocks.flushCanonicalSessionEditorChanges).toHaveBeenCalledWith(
      "session-1",
    );
    expect(
      mocks.flushCanonicalSessionEditorChanges.mock.invocationCallOrder[0],
    ).toBeLessThan(mocks.loadSessionShareSource.mock.invocationCallOrder[0]!);
    expect(mocks.toastSuccess).toHaveBeenCalledWith(
      "Desktop edits published. Sharing resumed.",
    );
  });

  it("keeps the conflict durable when a newer web revision wins the resolution CAS", async () => {
    mocks.syncStatus = "conflict";
    mocks.durableNote.contentRevision = 2;
    mocks.durableNote.webEditBase = {
      contentRevision: 1,
      title: "Planning",
      body: { type: "doc", content: [] },
    };
    mocks.loadSessionShareSyncState.mockResolvedValue({
      viewerUserId: USER_ID,
      shareId: SHARE_ID,
      sessionId: "session-1",
      acknowledgedContentRevision: 1,
      baselineSourceHash: "a".repeat(64),
      status: "conflict",
    });
    mocks.createOrReuseSessionShare.mockResolvedValueOnce({
      shareId: SHARE_ID,
      generalScope: "restricted",
      publicSlug: PUBLIC_SLUG,
      accessVersion: 1,
      wasCreated: false,
    });
    mocks.publishSessionShareSnapshot.mockRejectedValueOnce(
      new Error("snapshot conflict"),
    );
    renderShareButton();
    await openSharePopover();
    mocks.recordPublishedSessionShareState.mockClear();
    mocks.upsertDurableSharedNoteCache.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Keep desktop edits" }));

    await waitFor(() =>
      expect(mocks.toastError).toHaveBeenCalledWith(
        "Could not publish the desktop edits. Check the latest web copy and try again.",
      ),
    );
    expect(mocks.recordPublishedSessionShareState).not.toHaveBeenCalled();
    expect(mocks.upsertDurableSharedNoteCache).not.toHaveBeenCalled();
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
    await openSharePopover();
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

  it("abandons an open draft when the account changes", async () => {
    mocks.managedNote = null;
    const view = renderShareButtonView();

    const trigger = screen.getByRole("button", { name: "Share note" });
    fireEvent.click(trigger);

    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    expect(
      await screen.findByRole("heading", { name: "Share" }),
    ).not.toBeNull();

    mocks.auth.session = createSession(OTHER_USER_ID);
    view.rerender();

    expect(mocks.createOrReuseSessionShare).not.toHaveBeenCalled();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(screen.queryByTestId("share-popover")).toBeNull();
  });

  it("abandons an open draft when the active note changes", async () => {
    mocks.managedNote = null;
    const view = renderShareButtonView();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    expect(await screen.findByTestId("share-popover")).not.toBeNull();

    view.rerender("session-2");
    expect(screen.queryByTestId("share-popover")).toBeNull();
    expect(mocks.createOrReuseSessionShare).not.toHaveBeenCalled();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(mocks.toastError).not.toHaveBeenCalled();
  });

  it("stays silent when a note-switch remount closes a draft", async () => {
    mocks.managedNote = null;
    const queryClient = new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    });
    const element = (sessionId: string) => (
      <QueryClientProvider client={queryClient}>
        <SessionShareButton key={sessionId} sessionId={sessionId} />
      </QueryClientProvider>
    );
    const view = render(element("session-1"));

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    expect(await screen.findByTestId("share-popover")).not.toBeNull();

    view.rerender(element("session-2"));
    expect(mocks.createOrReuseSessionShare).not.toHaveBeenCalled();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(mocks.toastError).not.toHaveBeenCalled();
  });

  it("keeps controls visible when existing access fails to load", async () => {
    mocks.billing.isPaid = false;
    mocks.getSessionShareManagement.mockRejectedValueOnce(
      new Error("management unavailable"),
    );
    renderShareButtonView();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    expect(
      await screen.findByText("Access settings could not be loaded."),
    ).not.toBeNull();
    expect(
      screen.getByRole("textbox", { name: "Invitee email" }),
    ).not.toBeNull();
    expect(screen.queryByText("Loading access…")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Try again" }));

    await waitFor(() =>
      expect(mocks.getSessionShareManagement).toHaveBeenCalledTimes(2),
    );
    await waitFor(() =>
      expect(
        screen.queryByText("Access settings could not be loaded."),
      ).toBeNull(),
    );
    expect(screen.queryByText("Loading access…")).toBeNull();
  });

  it("does not resurface a dismissed upgrade prompt when the account returns", async () => {
    mocks.billing.isPaid = false;
    mocks.managedNote = null;
    const view = renderShareButtonView();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    expect(
      await screen.findByRole("heading", { name: "Share notes with others" }),
    ).not.toBeNull();

    mocks.auth.session = createSession(OTHER_USER_ID);
    view.rerender();
    expect(screen.queryByTestId("share-popover")).toBeNull();

    mocks.auth.session = createSession();
    view.rerender();

    expect(screen.queryByTestId("share-popover")).toBeNull();
    expect(
      screen.queryByRole("heading", { name: "Share notes with others" }),
    ).toBeNull();
  });

  it("abandons a billing wait when the account changes", async () => {
    mocks.managedNote = null;
    mocks.billing.isReady = false;
    const view = renderShareButtonView();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    expect(screen.queryByTestId("share-popover")).not.toBeNull();

    mocks.auth.session = createSession(OTHER_USER_ID);
    view.rerender();
    expect(screen.queryByTestId("share-popover")).toBeNull();

    mocks.auth.session = createSession();
    mocks.billing.isReady = true;
    view.rerender();

    await act(async () => {});
    expect(screen.queryByTestId("share-popover")).toBeNull();
    expect(mocks.loadSessionShareSource).not.toHaveBeenCalled();
    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
  });

  it("does not resurface an abandoned draft when the account returns", async () => {
    mocks.managedNote = null;
    const view = renderShareButtonView();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    expect(await screen.findByTestId("share-popover")).not.toBeNull();

    mocks.auth.session = createSession(OTHER_USER_ID);
    view.rerender();
    expect(screen.queryByTestId("share-popover")).toBeNull();

    mocks.auth.session = createSession();
    view.rerender();

    expect(screen.queryByTestId("share-popover")).toBeNull();
    expect(
      screen.queryByText("Access settings could not be loaded."),
    ).toBeNull();
    expect(mocks.toastError).not.toHaveBeenCalled();
  });

  it("does not flash the upgrade prompt while local share state is loading", async () => {
    mocks.billing.isPaid = false;
    mocks.managedNote = null;
    mocks.managedNoteLoading = true;
    const view = renderShareButtonView();

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    expect(await screen.findByTestId("share-popover")).not.toBeNull();
    expect(
      screen.queryByRole("heading", { name: "Share notes with others" }),
    ).toBeNull();
    expect(screen.queryByText("Loading access…")).toBeNull();

    mocks.auth.session = createSession(OTHER_USER_ID);
    view.rerender();
    expect(screen.queryByTestId("share-popover")).toBeNull();

    mocks.auth.session = createSession();
    view.rerender();

    expect(screen.queryByTestId("share-popover")).toBeNull();
    expect(screen.queryByText("Loading access…")).toBeNull();
  });

  it("offers invited, workspace, and link access and can restrict a link share", async () => {
    mocks.workspaces = [{ id: WORKSPACE_ID, name: "Fastrepl" }];
    mocks.management = defaultManagement({
      generalScope: "link",
      hasActiveLink: true,
    });
    renderShareButton();
    await openSharePopover();
    mocks.setSessionShareScope.mockClear();

    expect(screen.getByText("Only people invited")).not.toBeNull();
    expect(screen.getByText("Everyone in Fastrepl")).not.toBeNull();
    expect(screen.getByText("Anyone with the link")).not.toBeNull();
    expect(screen.queryByText("Public on the web")).toBeNull();

    fireEvent.click(screen.getByText("Only people invited"));

    await waitFor(() =>
      expect(mocks.setSessionShareScope).toHaveBeenCalledWith(
        expect.anything(),
        { shareId: SHARE_ID, scope: "restricted" },
      ),
    );
  });

  it("publishes before expanding an existing share to a workspace", async () => {
    mocks.workspaces = [{ id: WORKSPACE_ID, name: "Fastrepl" }];
    renderShareButton();
    await openSharePopover();
    mocks.events = [];
    mocks.markSessionShareActivated.mockClear();

    fireEvent.click(screen.getByText("Everyone in Fastrepl"));

    await waitFor(() =>
      expect(mocks.setSessionShareScope).toHaveBeenCalledWith(
        expect.anything(),
        {
          shareId: SHARE_ID,
          scope: "workspace",
          workspaceId: WORKSPACE_ID,
        },
      ),
    );
    expect(mocks.events.slice(0, 3)).toEqual(["load", "publish", "set-scope"]);
    expect(mocks.markSessionShareActivated).toHaveBeenCalledWith(
      USER_ID,
      SHARE_ID,
      "session-1",
    );
  });

  it("creates and copies a bearer link only after link access is selected", async () => {
    mocks.workspaces = [{ id: WORKSPACE_ID, name: "Fastrepl" }];
    mocks.managedNote = null;
    mocks.loadManagedSharedNoteForSession.mockResolvedValue(null);
    renderShareButton();
    await openSharePopover();

    expect(mocks.enableSessionShareLink).not.toHaveBeenCalled();
    expect(mocks.clipboardWriteText).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText("Anyone with the link"));

    await waitFor(() =>
      expect(mocks.toastSuccess).toHaveBeenCalledWith(
        "Anyone with the link can view. Link copied.",
      ),
    );
    expect(mocks.clipboardWriteText).toHaveBeenCalledOnce();
    expect(mocks.events.slice(0, 5)).toEqual([
      "load",
      "create",
      "management",
      "publish",
      "enable-link",
    ]);
    const copied = new URL(mocks.clipboardWriteText.mock.calls[0]![0]);
    expect(copied.pathname).toBe(`/share/link/${SHARE_ID}/`);
    expect(copied.hash).toBe(`#token=${TOKEN}`);
    expect(mocks.markSessionShareActivated).toHaveBeenCalledWith(
      USER_ID,
      SHARE_ID,
      "session-1",
    );
  });

  it("returns link access to invited-only when copying its token fails", async () => {
    mocks.workspaces = [{ id: WORKSPACE_ID, name: "Fastrepl" }];
    mocks.clipboardWriteText.mockRejectedValueOnce(new Error("clipboard"));
    renderShareButton();
    await openSharePopover();
    mocks.setSessionShareScope.mockClear();
    mocks.markSessionShareActivated.mockClear();

    fireEvent.click(screen.getByText("Anyone with the link"));

    await waitFor(() =>
      expect(mocks.setSessionShareScope).toHaveBeenCalledWith(
        expect.anything(),
        { shareId: SHARE_ID, scope: "restricted" },
      ),
    );
    expect(mocks.markSessionShareActivated).not.toHaveBeenCalled();
    expect(mocks.toastError).toHaveBeenCalledWith(
      "Could not update general access.",
    );
  });

  it("publishes before creating an invitation and sends its email", async () => {
    mocks.managedNote = null;
    mocks.loadManagedSharedNoteForSession.mockResolvedValue(null);
    renderShareButton();
    await openSharePopover();
    mocks.events = [];
    mocks.sendSessionAccessInvitationEmail.mockClear();

    fireEvent.change(screen.getByRole("textbox", { name: "Invitee email" }), {
      target: { value: "person@example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Invite" }));

    await waitFor(() =>
      expect(mocks.sendSessionAccessInvitationEmail).toHaveBeenCalledWith(
        expect.objectContaining({
          shareId: SHARE_ID,
          invitationId: INVITATION_ID,
          inviteToken: TOKEN,
          noteTitle: "Planning",
        }),
      ),
    );
    expect(mocks.events.slice(0, 5)).toEqual([
      "load",
      "create",
      "management",
      "publish",
      "create-invitation",
    ]);
    expect(mocks.markSessionShareActivated).toHaveBeenCalledWith(
      USER_ID,
      SHARE_ID,
      "session-1",
    );
    expect(mocks.toastSuccess).toHaveBeenCalledWith("Invitation sent.");
  });

  it("keeps an emailed invitation when the popover closes", async () => {
    let resolveEmail: (() => void) | undefined;
    mocks.sendSessionAccessInvitationEmail.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveEmail = resolve;
      }),
    );
    renderShareButton();
    await openSharePopover();

    fireEvent.change(screen.getByRole("textbox", { name: "Invitee email" }), {
      target: { value: "person@example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Invite" }));
    await waitFor(() =>
      expect(mocks.sendSessionAccessInvitationEmail).toHaveBeenCalledOnce(),
    );

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    await act(async () => {
      resolveEmail?.();
      await Promise.resolve();
    });

    expect(mocks.revokeSessionAccessInvitation).not.toHaveBeenCalled();
    expect(mocks.clipboardWriteText).not.toHaveBeenCalled();
  });

  it("does not revoke an invitation when dismissed email delivery fails", async () => {
    let rejectEmail!: (reason?: unknown) => void;
    mocks.sendSessionAccessInvitationEmail.mockReturnValueOnce(
      new Promise<void>((_resolve, reject) => {
        rejectEmail = reject;
      }),
    );
    renderShareButton();
    await openSharePopover();

    fireEvent.change(screen.getByRole("textbox", { name: "Invitee email" }), {
      target: { value: "person@example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Invite" }));
    await waitFor(() =>
      expect(mocks.sendSessionAccessInvitationEmail).toHaveBeenCalledOnce(),
    );

    fireEvent.click(screen.getByRole("button", { name: "Share note" }));
    await act(async () => {
      rejectEmail(new Error("mail unavailable"));
      await Promise.resolve();
    });

    expect(mocks.revokeSessionAccessInvitation).not.toHaveBeenCalled();
    expect(mocks.clipboardWriteText).not.toHaveBeenCalled();
  });

  it("copies the invite link when email delivery is unavailable", async () => {
    mocks.sendSessionAccessInvitationEmail.mockRejectedValueOnce(
      new Error("mail unavailable"),
    );
    renderShareButton();
    await openSharePopover();
    mocks.clipboardWriteText.mockClear();

    fireEvent.change(screen.getByRole("textbox", { name: "Invitee email" }), {
      target: { value: "person@example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Invite" }));

    await waitFor(() =>
      expect(mocks.clipboardWriteText).toHaveBeenCalledOnce(),
    );
    const copied = new URL(mocks.clipboardWriteText.mock.calls[0]![0]);
    expect(copied.pathname).toBe(`/share/invite/${INVITATION_ID}/`);
    expect(copied.hash).toBe(`#token=${TOKEN}`);
    expect(mocks.toastSuccess).toHaveBeenCalledWith(
      "Email unavailable. Invite link copied instead.",
    );
  });

  it("copies the account-gated note link", async () => {
    renderShareButton();
    await openSharePopover();
    mocks.clipboardWriteText.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Copy link" }));

    await waitFor(() =>
      expect(mocks.clipboardWriteText).toHaveBeenCalledOnce(),
    );
    const copied = new URL(mocks.clipboardWriteText.mock.calls[0]![0]);
    expect(copied.pathname).toBe(`/share/${SHARE_ID}/`);
    expect(copied.hash).toBe("");
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
    await openSharePopover();
    mocks.events = [];
    mocks.publishSessionShareSnapshot.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Remove" }));

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
    await openSharePopover();
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
    await openSharePopover();
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

    await openSharePopover();

    expect(mocks.publishSessionShareSnapshot).not.toHaveBeenCalled();
    expect(
      (screen.getByRole("button", { name: "Invite" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Remove" }));

    await waitFor(() =>
      expect(mocks.revokeSessionAccessGrant).toHaveBeenCalledOnce(),
    );
  });
});
