import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";

import { sharedButtonStyles } from "@/components/shared-note-viewer";
import { getShareRouteToken } from "@/lib/share-route-privacy";
import {
  createLinkShareHandoff,
  createPublicShareHandoff,
  createStableShareHandoff,
} from "@/lib/shared-note-api";
import {
  buildAccountShareDeepLink,
  buildShareHandoffDeepLink,
  type SharedNoteDesktopScheme,
} from "@/lib/shared-notes";
const styles = stylex.create({
  style1: {
    position: "relative",
  },
  style2: {
    display: {
      default: "none",
      "@media (width >= 40rem)": "inline",
    },
  },
  style3: {
    display: {
      default: null,
      "@media (width >= 40rem)": "none",
    },
  },
  style4: {
    flexBasis: "100%",
    textAlign: "right",
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: colors.mutedForeground,
  },
  tooltip: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: shadows.lg,
    color: colors.mutedForeground,
    fontSize: ".75rem",
    marginTop: ".5rem",
    opacity: {
      default: 0,
      [stylex.when.ancestor(":focus-within")]: 1,
      [stylex.when.ancestor(":hover")]: 1,
    },
    paddingBlock: ".375rem",
    paddingInline: ".625rem",
    pointerEvents: "none",
    position: "absolute",
    right: 0,
    top: "100%",
    transform: {
      default: "translateY(-2px)",
      [stylex.when.ancestor(":focus-within")]: "translateY(0)",
      [stylex.when.ancestor(":hover")]: "translateY(0)",
    },
    transitionDuration: "150ms",
    transitionProperty: "opacity, transform",
    whiteSpace: "nowrap",
  },
});
export function AccountSharedNoteActions({
  canEdit,
  scheme,
  shareId,
}: {
  canEdit: boolean;
  scheme: SharedNoteDesktopScheme;
  shareId: string;
}) {
  return (
    <SharedNoteActionButtons
      canEdit={canEdit}
      onOpen={() => {
        window.location.href = buildAccountShareDeepLink(shareId, scheme);
      }}
    />
  );
}
export function LinkSharedNoteActions({
  canEdit,
  pathname,
  scheme,
  shareId,
}: {
  canEdit: boolean;
  pathname: string;
  scheme: SharedNoteDesktopScheme;
  shareId: string;
}) {
  const handoffMutation = useMutation({
    mutationFn: async () => {
      const token = getShareRouteToken(pathname);
      if (!token) {
        throw new Error("shared note unavailable");
      }
      const handoff = await createLinkShareHandoff(shareId, token);
      if (!handoff) {
        throw new Error("shared note unavailable");
      }
      return handoff;
    },
    onSuccess: (handoff) => {
      window.location.href = buildShareHandoffDeepLink(
        handoff.requestId,
        scheme,
      );
    },
  });
  return (
    <SharedNoteActionButtons
      canEdit={canEdit}
      error={handoffMutation.isError}
      isPending={handoffMutation.isPending}
      onOpen={() => handoffMutation.mutate()}
    />
  );
}
export function StableSharedNoteActions({
  canEdit,
  scheme,
  shareId,
}: {
  canEdit: boolean;
  scheme: SharedNoteDesktopScheme;
  shareId: string;
}) {
  const handoffMutation = useMutation({
    mutationFn: async () => {
      const handoff = await createStableShareHandoff(shareId);
      if (!handoff) {
        throw new Error("shared note unavailable");
      }
      return handoff;
    },
    onSuccess: (handoff) => {
      window.location.href = buildShareHandoffDeepLink(
        handoff.requestId,
        scheme,
      );
    },
  });
  return (
    <SharedNoteActionButtons
      canEdit={canEdit}
      error={handoffMutation.isError}
      isPending={handoffMutation.isPending}
      onOpen={() => handoffMutation.mutate()}
    />
  );
}
export function PublicSharedNoteActions({
  canEdit,
  publicSlug,
  scheme,
}: {
  canEdit: boolean;
  publicSlug: string;
  scheme: SharedNoteDesktopScheme;
}) {
  const handoffMutation = useMutation({
    mutationFn: async () => {
      const handoff = await createPublicShareHandoff(publicSlug);
      if (!handoff) {
        throw new Error("shared note unavailable");
      }
      return handoff;
    },
    onSuccess: (handoff) => {
      window.location.href = buildShareHandoffDeepLink(
        handoff.requestId,
        scheme,
      );
    },
  });
  return (
    <SharedNoteActionButtons
      canEdit={canEdit}
      error={handoffMutation.isError}
      isPending={handoffMutation.isPending}
      onOpen={() => handoffMutation.mutate()}
    />
  );
}
function SharedNoteActionButtons({
  canEdit,
  error = false,
  isPending = false,
  onOpen,
}: {
  canEdit: boolean;
  error?: boolean;
  isPending?: boolean;
  onOpen: () => void;
}) {
  return (
    <>
      <div {...stylex.props(stylex.defaultMarker(), styles.style1)}>
        <button
          type="button"
          {...stylex.props([
            sharedButtonStyles.base,
            sharedButtonStyles.primary,
          ])}
          disabled={isPending}
          aria-describedby={canEdit ? "open-in-anarlog-tooltip" : undefined}
          onClick={onOpen}
        >
          <span {...stylex.props(styles.style2)}>
            {isPending ? "Opening…" : "Open in Anarlog"}
          </span>
          <span {...stylex.props(styles.style3)}>
            {isPending ? "Opening…" : "Open"}
          </span>
        </button>
        {canEdit && (
          <span
            id="open-in-anarlog-tooltip"
            role="tooltip"
            {...stylex.props(styles.tooltip)}
          >
            Open in Anarlog to edit
          </span>
        )}
      </div>
      {error && (
        <p {...stylex.props(styles.style4)} role="status">
          Anarlog couldn’t be opened. Try again.
        </p>
      )}
    </>
  );
}
