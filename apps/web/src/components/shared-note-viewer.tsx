import {
  AlertCircleIcon,
  CalendarDaysIcon,
  LoaderCircleIcon,
  RefreshCwIcon,
  UsersRoundIcon,
} from "lucide-react";

import { cn } from "@hypr/utils";

import {
  type SharedAttachmentResolver,
  SharedNoteDocument,
} from "@/components/shared-note-document";
import {
  type SharedNoteSnapshot,
  withoutDuplicateLeadingTitle,
} from "@/lib/shared-notes";

export const sharedPrimaryButtonClassName = cn([
  "inline-flex min-h-11 items-center justify-center rounded-full px-5",
  "bg-linear-to-t from-stone-600 to-stone-500 text-white",
  "font-mono text-sm font-medium transition-opacity hover:opacity-90",
  "focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:ring-offset-2 focus-visible:outline-hidden",
  "disabled:cursor-not-allowed disabled:opacity-50",
]);

export const sharedSecondaryButtonClassName = cn([
  "surface border-color-subtle inline-flex min-h-11 items-center justify-center rounded-full border px-5",
  "text-color hover:bg-surface-subtle font-mono text-sm font-medium transition-colors",
  "focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:ring-offset-2 focus-visible:outline-hidden",
]);

export function SharedNoteViewer({
  accessLabel,
  actions,
  documentContent,
  headerActions,
  notice,
  resolveAttachment,
  showTitle = true,
  snapshot,
}: {
  accessLabel: string;
  actions?: React.ReactNode;
  documentContent?: React.ReactNode;
  headerActions?: React.ReactNode;
  notice?: React.ReactNode;
  resolveAttachment?: SharedAttachmentResolver;
  showTitle?: boolean;
  snapshot: SharedNoteSnapshot;
}) {
  const body = withoutDuplicateLeadingTitle(snapshot.body, snapshot.title);

  return (
    <SharedNoteShell
      topActions={
        headerActions || actions ? (
          <div
            className={cn([
              "flex items-center gap-2",
              "[&>a]:min-h-9 [&>a]:px-4 [&>button]:min-h-9 [&>button]:px-4",
            ])}
          >
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
          <div
            className={cn([
              "text-color-muted flex min-w-0 flex-wrap items-center gap-2 text-xs",
              showTitle ? "mt-3" : "mb-6",
            ])}
          >
            <span className="surface border-color-subtle inline-flex min-h-8 items-center gap-1.5 rounded-full border px-3">
              <UsersRoundIcon className="size-3.5" aria-hidden="true" />
              {accessLabel}
            </span>
            <time
              className="surface border-color-subtle inline-flex min-h-8 items-center gap-1.5 rounded-full border px-3"
              dateTime={snapshot.publishedAt}
            >
              <CalendarDaysIcon className="size-3.5" aria-hidden="true" />
              {formatPublishedAt(snapshot.publishedAt)}
            </time>
          </div>
        </header>

        <div>
          {notice}
          {documentContent ?? (
            <SharedNoteDocument
              attachments={snapshot.attachments}
              document={body}
              resolveAttachment={resolveAttachment}
            />
          )}
        </div>
      </article>
    </SharedNoteShell>
  );
}

export function SharedNoteLoading() {
  return (
    <SharedNoteShell>
      <div
        className="surface border-color-subtle rounded-3xl border px-6 py-8 sm:px-10"
        aria-label="Loading shared note"
      >
        <LoaderCircleIcon
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
      icon={<AlertCircleIcon className="size-6" aria-hidden="true" />}
      title="This shared note isn’t available"
      description="The link may have expired, access may have changed, or the note may no longer be shared."
    />
  );
}

export function SharedNoteTransientError({ retry }: { retry?: () => void }) {
  return (
    <SharedNotePrompt
      icon={<AlertCircleIcon className="size-6" aria-hidden="true" />}
      title="We couldn’t load this shared note"
      description="Anarlog had a temporary problem loading the note. Please try again."
      actions={
        <button
          type="button"
          className={sharedPrimaryButtonClassName}
          onClick={retry ?? (() => window.location.reload())}
        >
          <RefreshCwIcon className="mr-2 size-4" aria-hidden="true" />
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
  return (
    <main
      className={cn([
        "bg-page text-color min-h-screen overflow-x-clip",
        "lg:[body:has([data-chat-panel-open])_&]:pr-[336px]",
      ])}
    >
      <header className="bg-page/95 border-color-subtle sticky top-0 z-40 flex h-14 items-center justify-between gap-4 border-b px-4 backdrop-blur-sm sm:px-6">
        <a
          href="/"
          aria-label="Anarlog home"
          className="font-hand text-color text-2xl leading-none font-semibold"
        >
          anarlog
        </a>
        {topActions ?? (
          <span className="text-color-muted font-mono text-xs">
            Shared with Anarlog
          </span>
        )}
      </header>
      <div
        className={cn([
          "mx-auto w-full max-w-[720px] px-5 py-8 sm:px-8 sm:py-10",
          "xl:has-[[data-comment-rail]]:max-w-[1028px] xl:has-[[data-comment-rail]]:pr-[308px]",
        ])}
      >
        {children}
      </div>
    </main>
  );
}

function formatPublishedAt(value: string) {
  return new Intl.DateTimeFormat("en-US", {
    day: "numeric",
    month: "short",
    timeZone: "UTC",
    year: "numeric",
  }).format(new Date(value));
}
