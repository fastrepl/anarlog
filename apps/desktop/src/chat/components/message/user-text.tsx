import { Facehash } from "facehash";
import { Building2Icon, StickyNoteIcon, type LucideIcon } from "lucide-react";
import { Fragment, useMemo } from "react";

import { cn } from "@hypr/utils";

import type { ContextRef } from "~/chat/context/entities";
import { getContextRefs } from "~/chat/context/refs";
import type { HyprUIMessage } from "~/chat/types";
import { getContactBgClass } from "~/contacts/shared";
import * as main from "~/store/tinybase/store/main";
import { useTabs } from "~/store/zustand/tabs";

type MentionCandidate = {
  key: string;
  kind: ContextRef["kind"];
  entityId: string;
  displayLabel: string;
  tokens: string[];
};

type MentionSegment =
  | { type: "text"; text: string }
  | { type: "mention"; mention: MentionCandidate };

function uniqueLabels(labels: Array<string | null | undefined>): string[] {
  const seen = new Set<string>();
  const result: string[] = [];

  for (const label of labels) {
    if (typeof label !== "string") {
      continue;
    }

    const trimmed = label.trim();
    if (!trimmed || seen.has(trimmed)) {
      continue;
    }

    seen.add(trimmed);
    result.push(trimmed);
  }

  return result;
}

function getMentionCandidate(
  ref: ContextRef,
  store: ReturnType<typeof main.UI.useStore>,
): MentionCandidate | null {
  if (ref.kind === "human") {
    const row = store?.getRow("humans", ref.humanId) ?? {};
    const labels = uniqueLabels([
      ref.label,
      typeof row.name === "string" ? row.name : null,
      typeof row.email === "string" ? row.email : null,
    ]);

    if (labels.length === 0) {
      return null;
    }

    return {
      key: ref.key,
      kind: ref.kind,
      entityId: ref.humanId,
      displayLabel: labels[0],
      tokens: labels.map((label) => `@${label}`),
    };
  }

  if (ref.kind === "session") {
    const row = store?.getRow("sessions", ref.sessionId) ?? {};
    const labels = uniqueLabels([
      ref.label,
      typeof row.title === "string" ? row.title : null,
    ]);

    if (labels.length === 0) {
      return null;
    }

    return {
      key: ref.key,
      kind: ref.kind,
      entityId: ref.sessionId,
      displayLabel: labels[0],
      tokens: labels.map((label) => `@${label}`),
    };
  }

  const row = store?.getRow("organizations", ref.organizationId) ?? {};
  const labels = uniqueLabels([
    ref.label,
    typeof row.name === "string" ? row.name : null,
  ]);

  if (labels.length === 0) {
    return null;
  }

  return {
    key: ref.key,
    kind: ref.kind,
    entityId: ref.organizationId,
    displayLabel: labels[0],
    tokens: labels.map((label) => `@${label}`),
  };
}

export function buildUserMessageSegments(
  text: string,
  mentions: MentionCandidate[],
): MentionSegment[] {
  if (!text) {
    return [];
  }

  const segments: MentionSegment[] = [];
  let cursor = 0;

  while (cursor < text.length) {
    let match:
      | {
          mention: MentionCandidate;
          start: number;
          end: number;
          tokenLength: number;
        }
      | undefined;

    for (const mention of mentions) {
      for (const token of mention.tokens) {
        const start = text.indexOf(token, cursor);
        if (start === -1) {
          continue;
        }

        if (
          !match ||
          start < match.start ||
          (start === match.start && token.length > match.tokenLength)
        ) {
          match = {
            mention,
            start,
            end: start + token.length,
            tokenLength: token.length,
          };
        }
      }
    }

    if (!match) {
      segments.push({ type: "text", text: text.slice(cursor) });
      break;
    }

    if (match.start > cursor) {
      segments.push({ type: "text", text: text.slice(cursor, match.start) });
    }

    segments.push({ type: "mention", mention: match.mention });
    cursor = match.end;
  }

  return segments;
}

function UserMentionChip({ mention }: { mention: MentionCandidate }) {
  const openNew = useTabs((state) => state.openNew);

  const handleClick = () => {
    if (mention.kind === "session") {
      openNew({ type: "sessions", id: mention.entityId });
      return;
    }

    if (mention.kind === "human") {
      openNew({ type: "humans", id: mention.entityId });
      return;
    }

    openNew({ type: "organizations", id: mention.entityId });
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      className={cn([
        "mention cursor-pointer bg-transparent p-0 align-baseline",
        "hover:opacity-80",
      ])}
    >
      <MentionAvatar mention={mention} />
      <span className="mention-text">{mention.displayLabel}</span>
    </button>
  );
}

function MentionAvatar({ mention }: { mention: MentionCandidate }) {
  if (mention.kind === "human") {
    const bgClass = getContactBgClass(mention.displayLabel);
    return (
      <span className={cn(["mention-avatar", bgClass])}>
        <Facehash
          name={mention.displayLabel}
          size={16}
          showInitial={true}
          interactive={false}
          colorClasses={[bgClass]}
        />
      </span>
    );
  }

  const Icon: LucideIcon =
    mention.kind === "session" ? StickyNoteIcon : Building2Icon;

  return (
    <span className="mention-avatar mention-avatar-icon">
      <Icon className="mention-inline-icon" />
    </span>
  );
}

export function UserMessageText({ message }: { message: HyprUIMessage }) {
  const store = main.UI.useStore(main.STORE_ID);
  const text = message.parts
    .filter(
      (
        part,
      ): part is Extract<(typeof message.parts)[number], { type: "text" }> =>
        part.type === "text",
    )
    .map((part) => part.text)
    .join("\n");

  const mentions = useMemo(
    () =>
      getContextRefs(message.metadata)
        .map((ref) => getMentionCandidate(ref, store))
        .filter((mention): mention is MentionCandidate => mention !== null),
    [message.metadata, store],
  );

  const segments = useMemo(
    () => buildUserMessageSegments(text, mentions),
    [text, mentions],
  );

  return (
    <div className="px-0.5 py-1 leading-7 break-words whitespace-pre-wrap">
      {segments.map((segment, index) => {
        if (segment.type === "text") {
          return <Fragment key={`text-${index}`}>{segment.text}</Fragment>;
        }

        return (
          <UserMentionChip
            key={`${segment.mention.key}-${index}`}
            mention={segment.mention}
          />
        );
      })}
    </div>
  );
}
