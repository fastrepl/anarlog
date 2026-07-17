import {
  AlertCircleIcon,
  LoaderCircleIcon,
  RefreshCwIcon,
  UsersRoundIcon,
} from "lucide-react";

import { cn } from "@hypr/utils";

import { AnarlogLogo } from "@/components/anarlog-logo";
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
  resolveAttachment,
  snapshot,
}: {
  accessLabel: string;
  actions?: React.ReactNode;
  resolveAttachment?: SharedAttachmentResolver;
  snapshot: SharedNoteSnapshot;
}) {
  const body = withoutDuplicateLeadingTitle(snapshot.body, snapshot.title);

  return (
    <SharedNoteShell>
      <article className="surface border-color-subtle overflow-hidden rounded-3xl border shadow-sm">
        <header className="border-color-subtle border-b px-6 py-7 sm:px-10 sm:py-10">
          <div className="text-color-muted flex items-center gap-2 text-sm">
            <UsersRoundIcon className="size-4" aria-hidden="true" />
            <span>{accessLabel}</span>
            <span aria-hidden="true">·</span>
            <time dateTime={snapshot.publishedAt}>
              {formatPublishedAt(snapshot.publishedAt)}
            </time>
          </div>
          <h1 className="text-color mt-5 font-mono text-3xl font-medium text-balance sm:text-4xl">
            {snapshot.title || "Untitled note"}
          </h1>
        </header>

        <div className="px-6 py-7 sm:px-10 sm:py-10">
          <SharedNoteDocument
            attachments={snapshot.attachments}
            document={body}
            resolveAttachment={resolveAttachment}
          />
        </div>
      </article>

      {actions && (
        <aside className="surface-subtle border-color-subtle mt-6 rounded-2xl border p-5 sm:flex sm:items-center sm:justify-between sm:gap-6">
          <div>
            <h2 className="text-color font-mono text-base font-medium">
              Keep this note close
            </h2>
            <p className="text-color-muted mt-1 text-sm leading-6">
              Open it in Anarlog, or try the local-first desktop app.
            </p>
          </div>
          <div className="mt-4 flex flex-wrap gap-3 sm:mt-0">{actions}</div>
        </aside>
      )}
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

function SharedNoteShell({ children }: { children: React.ReactNode }) {
  return (
    <main className="bg-page text-color min-h-screen px-4 py-5 sm:px-6 sm:py-8">
      <div className="mx-auto w-full max-w-[760px]">
        <header className="mb-8 flex items-center justify-between gap-4 px-1">
          <a href="/" aria-label="Anarlog home">
            <AnarlogLogo className="h-8 w-auto" />
          </a>
          <span className="text-color-muted font-mono text-xs">
            Shared with Anarlog
          </span>
        </header>
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
