import {
  ArrowsClockwise,
  CalendarDots,
  CircleNotch,
  Users,
  WarningCircle,
} from "@phosphor-icons/react";
import { useSyncExternalStore } from "react";

import { Avatar } from "@anlg/ui/components/avatar";
import { cn } from "@anlg/utils";

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

export const sharedPrimaryButtonClassName = cn([
  "inline-flex min-h-9 items-center justify-center rounded-full bg-stone-900 px-4 text-white",
  "text-sm font-semibold transition-colors hover:bg-stone-800",
  "focus-visible:ring-2 focus-visible:ring-stone-900 focus-visible:ring-offset-2 focus-visible:outline-hidden",
  "disabled:cursor-not-allowed disabled:opacity-50",
]);

export const sharedSecondaryButtonClassName = cn([
  "surface border-color-subtle inline-flex min-h-9 items-center justify-center rounded-full border px-4",
  "text-color hover:bg-surface-subtle text-sm font-medium transition-colors",
  "focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:ring-offset-2 focus-visible:outline-hidden",
]);

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
          <div className="flex items-center gap-2">
            {headerActions}
            {actions}
          </div>
        ) : undefined
      }
    >
      <article className="xl:overflow-visible">
        <header className="mb-6">
          {showTitle && (
            <h1 className="text-color text-2xl leading-[1.875rem] font-semibold text-balance">
              {snapshot.title || "Untitled note"}
            </h1>
          )}
          {meetingMetadata ? (
            <SharedNoteMeetingMetadata
              className={showTitle ? "mt-3" : "mb-6"}
              meetingAt={meetingMetadata.meetingAt}
              participants={meetingMetadata.participants}
            />
          ) : (
            <div
              className={cn([
                "flex min-w-0 flex-wrap items-center gap-x-4 gap-y-1 text-xs text-stone-500",
                showTitle ? "mt-3" : "mb-6",
              ])}
            >
              <span className="inline-flex min-h-7 items-center gap-1.5">
                <Users className="size-3.5" aria-hidden="true" />
                {accessLabel}
              </span>
              <time
                className="inline-flex min-h-7 items-center gap-1.5"
                dateTime={snapshot.publishedAt}
                title={`Published ${formatSharedNotePublishedAt(snapshot.publishedAt)}`}
              >
                <CalendarDots className="size-3.5" aria-hidden="true" />
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
  className,
  meetingAt,
  participants,
}: Pick<SharedNotePreview, "meetingAt" | "participants"> & {
  className?: string;
}) {
  const presentation = createSharedNoteParticipantPresentation(participants);

  return (
    <div
      className={cn([
        "flex min-w-0 flex-wrap items-center gap-x-2 gap-y-2 text-sm text-stone-500",
        className,
      ])}
    >
      {presentation.participantCount > 0 ? (
        <div
          aria-label={`${presentation.participantCount} meeting participants`}
          className="flex shrink-0 items-center"
        >
          {presentation.avatarParticipants.map((participant, index) => (
            <span
              className={cn(["relative inline-flex", index > 0 && "-ml-2"])}
              key={participant}
              style={{
                zIndex: presentation.avatarParticipants.length - index,
              }}
            >
              <Avatar label={participant} seed={participant} size={30} />
            </span>
          ))}
        </div>
      ) : null}
      <span className="font-medium text-stone-700">{presentation.label}</span>
      <span aria-hidden="true">·</span>
      <time dateTime={meetingAt}>{formatSharedNoteMeetingAt(meetingAt)}</time>
    </div>
  );
}

export function SharedNoteLoading() {
  return (
    <SharedNoteShell>
      <div
        className="surface border-color-subtle rounded-3xl border px-6 py-8 sm:px-10"
        aria-label="Loading shared note"
      >
        <CircleNotch
          className="text-color-muted mb-6 size-5 animate-spin"
          aria-hidden="true"
        />
        <div className="surface-subtle h-7 w-3/5 animate-pulse rounded-lg" />
        <div className="surface-subtle mt-6 h-4 w-full animate-pulse rounded" />
        <div className="surface-subtle mt-3 h-4 w-4/5 animate-pulse rounded" />
      </div>
    </SharedNoteShell>
  );
}

export function SharedNoteUnavailable() {
  return (
    <SharedNotePrompt
      icon={<WarningCircle className="size-6" aria-hidden="true" />}
      title="This shared note isn’t available"
      description="The link may have expired, access may have changed, or the note may no longer be shared."
    />
  );
}

export function SharedNoteTransientError({ retry }: { retry?: () => void }) {
  return (
    <SharedNotePrompt
      icon={<WarningCircle className="size-6" aria-hidden="true" />}
      title="We couldn’t load this shared note"
      description="Anarlog had a temporary problem loading the note. Please try again."
      actions={
        <button
          type="button"
          className={sharedPrimaryButtonClassName}
          onClick={retry ?? (() => window.location.reload())}
        >
          <ArrowsClockwise className="mr-2 size-4" aria-hidden="true" />
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
      <section className="surface border-color-subtle rounded-3xl border px-6 py-12 text-center sm:px-10">
        {icon && (
          <div className="text-color-muted mx-auto mb-4 flex justify-center">
            {icon}
          </div>
        )}
        <h1 className="text-color font-mono text-2xl font-medium">{title}</h1>
        <p className="text-color-muted mx-auto mt-3 max-w-lg text-base leading-7">
          {description}
        </p>
        {actions && (
          <div className="mt-7 flex flex-wrap justify-center gap-3">
            {actions}
          </div>
        )}
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
    <main className="min-h-screen overflow-x-clip bg-white text-stone-900">
      <header
        className={cn([
          "sticky top-0 z-40 flex h-14 items-center justify-between gap-4 border-b border-stone-200 bg-white/95 px-4 backdrop-blur-sm sm:px-6",
          "transition-transform duration-200 will-change-transform motion-reduce:transition-none",
          headerHidden && "-translate-y-full",
        ])}
      >
        <a href="/" aria-label="Anarlog home" className="inline-flex">
          <AnarlogLogo className="h-7 w-auto" />
        </a>
        {topActions ?? (
          <span className="text-xs text-stone-500">Shared with Anarlog</span>
        )}
      </header>
      <div
        className={cn([
          "mx-auto w-full max-w-[720px] px-5 py-8 sm:px-8 sm:py-10",
          "xl:has-[[data-comment-rail]]:max-w-[1028px] xl:has-[[data-comment-rail]]:pr-[372px] xl:has-[[data-comment-rail]]:pl-0",
        ])}
      >
        {children}
      </div>
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
    window.addEventListener("scroll", handleHeaderScroll, { passive: true });
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
