import {
  ArrowsClockwise,
  CalendarDots,
  CircleNotch,
  Users,
  WarningCircle,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useSyncExternalStore } from "react";

import { colors, fonts, media, radii } from "@anlg/design-system/tokens.stylex";
import { Avatar } from "@anlg/ui/components/avatar";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

import { AnarlogLogo } from "@/components/anarlog-logo";
import { SharedNoteAudioPlayer } from "@/components/shared-note-audio-player";
import {
  type SharedAttachmentResolver,
  SharedNoteDocument,
} from "@/components/shared-note-document";
import { useMountEffect } from "@/hooks/useMountEffect";
import { capturePrivateRouteEvent } from "@/lib/private-route-analytics";
import {
  createSharedNoteParticipantPresentation,
  findFeaturedSharedNoteAudio,
  formatSharedNoteMeetingAt,
  formatSharedNotePublishedAt,
} from "@/lib/shared-note-presentation";
import {
  type SharedNotePreview,
  type SharedNoteSnapshot,
  withoutDuplicateLeadingTitle,
} from "@/lib/shared-notes";

const pulse = stylex.keyframes({
  "0%, 100%": { opacity: 1 },
  "50%": { opacity: 0.5 },
});

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  style1: {
    display: "flex",
    alignItems: "center",
    gap: ".5rem",
  },
  style2: {
    overflow: {
      default: null,
      "@media (width >= 80rem)": "visible",
    },
  },
  style3: {
    marginBottom: "1.5rem",
  },
  style4: {
    color: colors.foreground,
    fontSize: "1.5rem",
    lineHeight: "1.875rem",
    fontWeight: 600,
    textWrap: "balance",
  },
  style6: {
    display: "inline-flex",
    minHeight: "1.75rem",
    alignItems: "center",
    gap: ".375rem",
  },
  style7: {
    width: ".875rem",
    height: ".875rem",
  },
  style8: {
    display: "flex",
    flexShrink: 0,
    alignItems: "center",
  },
  style9: {
    fontWeight: 500,
    color: "#44403c",
  },
  style10: {
    backgroundColor: colors.card,
    borderRadius: "1.5rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: colors.border,
    paddingInline: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2.5rem",
    },
    paddingBlock: "2rem",
  },
  style11: {
    marginBottom: "1.5rem",
    width: "1.25rem",
    height: "1.25rem",
    animationDuration: "1s",
    animationTimingFunction: "linear",
    animationIterationCount: "infinite",
    animationName: {
      default: spin,
      [media.reducedMotion]: "none",
    },
  },
  style12: {
    backgroundColor: colors.muted,
    height: "1.75rem",
    width: "60%",
    animationDuration: "2s",
    animationTimingFunction: "cubic-bezier(.4, 0, .6, 1)",
    animationIterationCount: "infinite",
    animationName: {
      default: pulse,
      [media.reducedMotion]: "none",
    },
    borderRadius: ".5rem",
  },
  style13: {
    backgroundColor: colors.muted,
    marginTop: "1.5rem",
    height: "1rem",
    width: "100%",
    animationDuration: "2s",
    animationTimingFunction: "cubic-bezier(.4, 0, .6, 1)",
    animationIterationCount: "infinite",
    animationName: {
      default: pulse,
      [media.reducedMotion]: "none",
    },
    borderRadius: ".25rem",
  },
  style14: {
    backgroundColor: colors.muted,
    marginTop: ".75rem",
    height: "1rem",
    width: "80%",
    animationDuration: "2s",
    animationTimingFunction: "cubic-bezier(.4, 0, .6, 1)",
    animationIterationCount: "infinite",
    animationName: {
      default: pulse,
      [media.reducedMotion]: "none",
    },
    borderRadius: ".25rem",
  },
  style15: {
    width: "1.5rem",
    height: "1.5rem",
  },
  style16: {
    marginRight: ".5rem",
    width: "1rem",
    height: "1rem",
  },
  style17: {
    backgroundColor: colors.card,
    borderRadius: "1.5rem",
    borderStyle: "solid",
    borderWidth: "1px",
    borderColor: colors.border,
    paddingInline: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2.5rem",
    },
    paddingBlock: "3rem",
    textAlign: "center",
  },
  style18: {
    marginInline: "auto",
    marginBottom: "1rem",
    display: "flex",
    justifyContent: "center",
    color: colors.mutedForeground,
  },
  style19: {
    color: colors.foreground,
    fontFamily: fonts.mono,
    fontSize: "1.5rem",
    lineHeight: "2rem",
    fontWeight: 500,
  },
  style20: {
    marginInline: "auto",
    marginTop: ".75rem",
    maxWidth: "32rem",
    color: colors.mutedForeground,
    fontSize: "1rem",
    lineHeight: "1.75rem",
  },
  style21: {
    marginTop: "1.75rem",
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "center",
    gap: ".75rem",
  },
  style22: {
    minHeight: "100vh",
    overflowX: "clip",
    backgroundColor: colors.card,
    color: colors.foreground,
  },
  style23: {
    display: "inline-flex",
  },
  style24: {
    height: "1.75rem",
    width: "auto",
  },
  style25: {
    fontSize: ".75rem",
    lineHeight: "1rem",
    color: "#78716c",
  },
  meetingMetadata: {
    alignItems: "center",
    color: colors.mutedForeground,
    columnGap: ".5rem",
    display: "flex",
    flexWrap: "wrap",
    fontSize: ".875rem",
    minWidth: 0,
    rowGap: ".5rem",
  },
  meetingMetadataAfterTitle: {
    marginTop: ".75rem",
  },
  meetingMetadataWithoutTitle: {
    marginBottom: "1.5rem",
  },
  participantAvatar: {
    display: "inline-flex",
    position: "relative",
  },
  participantAvatarLayer: (zIndex: number) => ({
    zIndex,
  }),
  participantAvatarOverlapping: {
    marginLeft: "-.5rem",
  },
  publishedMetadata: {
    alignItems: "center",
    color: colors.mutedForeground,
    columnGap: "1rem",
    display: "flex",
    flexWrap: "wrap",
    fontSize: ".75rem",
    minWidth: 0,
    rowGap: ".25rem",
  },
  header: {
    alignItems: "center",
    backdropFilter: "blur(4px)",
    backgroundColor: `color-mix(in oklab, ${colors.card} 95%, transparent)`,
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    display: "flex",
    gap: "1rem",
    height: "3.5rem",
    justifyContent: "space-between",
    left: 0,
    paddingInline: {
      default: "1rem",
      "@media (min-width: 40rem)": "1.5rem",
    },
    position: "sticky",
    top: 0,
    transform: {
      default: "translateY(0)",
      "@media (prefers-reduced-motion: reduce)": "none",
    },
    transitionDuration: {
      default: "200ms",
      "@media (prefers-reduced-motion: reduce)": "0ms",
    },
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    willChange: "transform",
    zIndex: 1,
  },
  headerHidden: {
    transform: {
      default: "translateY(-100%)",
      "@media (prefers-reduced-motion: reduce)": "none",
    },
  },
  shellContent: {
    marginInline: "auto",
    maxWidth: {
      default: "720px",
      "@media (min-width: 80rem)": {
        default: "720px",
        ":is(:has([data-comment-rail]))": "1028px",
      },
    },
    paddingBlock: {
      default: "2rem",
      "@media (min-width: 40rem)": "2.5rem",
    },
    paddingLeft: {
      default: "1.25rem",
      "@media (min-width: 40rem)": "2rem",
      "@media (min-width: 80rem)": {
        default: "2rem",
        ":is(:has([data-comment-rail]))": 0,
      },
    },
    paddingRight: {
      default: "1.25rem",
      "@media (min-width: 40rem)": "2rem",
      "@media (min-width: 80rem)": {
        default: "2rem",
        ":is(:has([data-comment-rail]))": "372px",
      },
    },
    width: "100%",
  },
});
export const sharedButtonStyles = stylex.create({
  base: {
    alignItems: "center",
    borderRadius: radii.full,
    boxShadow: {
      default: null,
      ":focus-visible": `0 0 0 2px ${colors.ring}, 0 0 0 4px ${colors.card}`,
    },
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    display: "inline-flex",
    fontSize: ".875rem",
    justifyContent: "center",
    minHeight: "2.25rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    outline: {
      default: null,
      ":focus-visible": "none",
    },
    paddingInline: "1rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color, border-color, color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  primary: {
    backgroundColor: {
      default: colors.foreground,
      ":hover": colors.primary,
    },
    color: colors.primaryForeground,
    fontWeight: 600,
  },
  secondary: {
    backgroundColor: {
      default: colors.card,
      ":hover": colors.muted,
    },
    borderColor: colors.border,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.foreground,
    fontWeight: 500,
  },
});
export function SharedNoteViewer({
  accessLabel,
  actions,
  documentContent,
  headerActions,
  meetingMetadata,
  notice,
  resolveAttachment,
  showTitle = true,
  snapshot,
}: {
  accessLabel: string;
  actions?: React.ReactNode;
  documentContent?: React.ReactNode;
  headerActions?: React.ReactNode;
  meetingMetadata?: Pick<
    SharedNotePreview,
    "meetingAt" | "participants"
  > | null;
  notice?: React.ReactNode;
  resolveAttachment?: SharedAttachmentResolver;
  showTitle?: boolean;
  snapshot: SharedNoteSnapshot;
}) {
  const body = withoutDuplicateLeadingTitle(snapshot.body, snapshot.title);
  const featuredAudio = findFeaturedSharedNoteAudio(snapshot.attachments);
  useMountEffect(() => {
    capturePrivateRouteEvent("shared_note_opened", {
      has_audio: Boolean(featuredAudio),
      has_collaboration_actions: Boolean(actions),
    });
  });
  return (
    <SharedNoteShell
      topActions={
        headerActions || actions ? (
          <div {...stylex.props(styles.style1)}>
            {headerActions}
            {actions}
          </div>
        ) : undefined
      }
    >
      <article {...stylex.props(styles.style2)}>
        <header {...stylex.props(styles.style3)}>
          {showTitle && (
            <h1 {...stylex.props(styles.style4)}>
              {snapshot.title || "Untitled note"}
            </h1>
          )}
          {meetingMetadata ? (
            <SharedNoteMeetingMetadata
              sx={
                showTitle
                  ? styles.meetingMetadataAfterTitle
                  : styles.meetingMetadataWithoutTitle
              }
              meetingAt={meetingMetadata.meetingAt}
              participants={meetingMetadata.participants}
            />
          ) : (
            <div
              {...stylex.props([
                styles.publishedMetadata,
                showTitle
                  ? styles.meetingMetadataAfterTitle
                  : styles.meetingMetadataWithoutTitle,
              ])}
            >
              <span {...stylex.props(styles.style6)}>
                <Users {...stylex.props(styles.style7)} aria-hidden="true" />
                {accessLabel}
              </span>
              <time
                {...stylex.props(styles.style6)}
                dateTime={snapshot.publishedAt}
                title={`Published ${formatSharedNotePublishedAt(snapshot.publishedAt)}`}
              >
                <CalendarDots
                  {...stylex.props(styles.style7)}
                  aria-hidden="true"
                />
                {formatSharedNotePublishedAt(snapshot.publishedAt)}
              </time>
            </div>
          )}
        </header>

        <div>
          {notice}
          {featuredAudio ? (
            <SharedNoteAudioPlayer
              attachment={featuredAudio}
              resolve={resolveAttachment}
            />
          ) : null}
          {documentContent ?? (
            <SharedNoteDocument
              attachments={snapshot.attachments}
              document={body}
              excludedAttachmentIds={
                featuredAudio ? [featuredAudio.id] : undefined
              }
              resolveAttachment={resolveAttachment}
            />
          )}
        </div>
      </article>
    </SharedNoteShell>
  );
}
function SharedNoteMeetingMetadata({
  meetingAt,
  participants,
  sx,
}: Pick<SharedNotePreview, "meetingAt" | "participants"> & {} & StyleXProps) {
  const presentation = createSharedNoteParticipantPresentation(participants);
  return (
    <div {...mergeStyleXProps([styles.meetingMetadata, sx])}>
      {presentation.participantCount > 0 ? (
        <div
          aria-label={`${presentation.participantCount} meeting participants`}
          {...stylex.props(styles.style8)}
        >
          {presentation.avatarParticipants.map((participant, index) => (
            <span
              {...stylex.props(
                styles.participantAvatar,
                styles.participantAvatarLayer(
                  presentation.avatarParticipants.length - index,
                ),
                index > 0 && styles.participantAvatarOverlapping,
              )}
              key={participant}
            >
              <Avatar label={participant} seed={participant} size={30} />
            </span>
          ))}
        </div>
      ) : null}
      <span {...stylex.props(styles.style9)}>{presentation.label}</span>
      <span aria-hidden="true">·</span>
      <time dateTime={meetingAt}>{formatSharedNoteMeetingAt(meetingAt)}</time>
    </div>
  );
}
export function SharedNoteLoading() {
  return (
    <SharedNoteShell>
      <div {...stylex.props(styles.style10)} aria-label="Loading shared note">
        <CircleNotch {...stylex.props(styles.style11)} aria-hidden="true" />
        <div {...stylex.props(styles.style12)} />
        <div {...stylex.props(styles.style13)} />
        <div {...stylex.props(styles.style14)} />
      </div>
    </SharedNoteShell>
  );
}
export function SharedNoteUnavailable() {
  return (
    <SharedNotePrompt
      icon={
        <WarningCircle {...stylex.props(styles.style15)} aria-hidden="true" />
      }
      title="This shared note isn’t available"
      description="The link may have expired, access may have changed, or the note may no longer be shared."
    />
  );
}
export function SharedNoteTransientError({ retry }: { retry?: () => void }) {
  return (
    <SharedNotePrompt
      icon={
        <WarningCircle {...stylex.props(styles.style15)} aria-hidden="true" />
      }
      title="We couldn’t load this shared note"
      description="Anarlog had a temporary problem loading the note. Please try again."
      actions={
        <button
          type="button"
          {...stylex.props(sharedButtonStyles.base, sharedButtonStyles.primary)}
          onClick={retry ?? (() => window.location.reload())}
        >
          <ArrowsClockwise
            {...stylex.props(styles.style16)}
            aria-hidden="true"
          />
          Try again
        </button>
      }
    />
  );
}
export function SharedNotePrompt({
  actions,
  description,
  icon,
  title,
}: {
  actions?: React.ReactNode;
  description: string;
  icon?: React.ReactNode;
  title: string;
}) {
  return (
    <SharedNoteShell>
      <section {...stylex.props(styles.style17)}>
        {icon && <div {...stylex.props(styles.style18)}>{icon}</div>}
        <h1 {...stylex.props(styles.style19)}>{title}</h1>
        <p {...stylex.props(styles.style20)}>{description}</p>
        {actions && <div {...stylex.props(styles.style21)}>{actions}</div>}
      </section>
    </SharedNoteShell>
  );
}
function SharedNoteShell({
  children,
  topActions,
}: {
  children: React.ReactNode;
  topActions?: React.ReactNode;
}) {
  const headerHidden = useSyncExternalStore(
    subscribeHeaderVisibility,
    getHeaderHidden,
    () => false,
  );
  return (
    <main {...stylex.props(styles.style22)}>
      <header
        {...stylex.props(styles.header, headerHidden && styles.headerHidden)}
      >
        <a href="/" aria-label="Anarlog home" {...stylex.props(styles.style23)}>
          <AnarlogLogo sx={styles.style24} />
        </a>
        {topActions ?? (
          <span {...stylex.props(styles.style25)}>Shared with Anarlog</span>
        )}
      </header>
      <div {...stylex.props(styles.shellContent)}>{children}</div>
    </main>
  );
}
let headerHidden = false;
let lastHeaderScrollY = 0;
const headerVisibilityListeners = new Set<() => void>();
function handleHeaderScroll() {
  const nextScrollY = window.scrollY;
  const nextHidden =
    nextScrollY > 80 &&
    (nextScrollY > lastHeaderScrollY + 8
      ? true
      : nextScrollY < lastHeaderScrollY - 8
        ? false
        : headerHidden);
  lastHeaderScrollY = nextScrollY;
  if (nextHidden === headerHidden) return;
  headerHidden = nextHidden;
  headerVisibilityListeners.forEach((listener) => listener());
}
function subscribeHeaderVisibility(onChange: () => void) {
  headerVisibilityListeners.add(onChange);
  if (headerVisibilityListeners.size === 1) {
    lastHeaderScrollY = window.scrollY;
    window.addEventListener("scroll", handleHeaderScroll, {
      passive: true,
    });
  }
  return () => {
    headerVisibilityListeners.delete(onChange);
    if (headerVisibilityListeners.size === 0) {
      window.removeEventListener("scroll", handleHeaderScroll);
      headerHidden = false;
    }
  };
}
function getHeaderHidden() {
  return headerHidden;
}
