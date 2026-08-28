import { Trans, useLingui } from "@lingui/react/macro";
import { MagnifyingGlass, Plus } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useCallback, useMemo, useRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import type { EventParticipant } from "@anlg/store";
import { Checkbox } from "@anlg/ui/components/ui/checkbox";
import {
  AppFloatingPanel,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@anlg/ui/components/ui/popover";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { preserveScrollPosition } from "./viewport-hooks";

import { trackAnalyticsEvent } from "~/analytics";
import { useSessionEventParticipants } from "~/calendar/queries";
import { ContactImage } from "~/contacts/contact-avatar";
import { createHuman, useHumans } from "~/contacts/queries";
import { ContactFacehash } from "~/contacts/shared";
import {
  addSessionParticipant,
  useSession,
  useSessionParticipants,
} from "~/session/queries";
import type { Segment } from "~/stt/live-segment";
import { assignTranscriptSpeaker } from "~/stt/queries";

export type AssignmentMode = "all" | "segment";

export function SpeakerAssignPopover({
  segment,
  transcriptId,
  sessionId,
  color,
  label,
  className,
  onAssigned,
}: {
  segment: Segment;
  transcriptId: string;
  sessionId?: string;
  color: string;
  label: string;
  className?: string;
  onAssigned?: (humanId: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);

  const handleOpenChange = useCallback((nextOpen: boolean) => {
    setOpen(nextOpen);
  }, []);

  const handleAssign = useCallback(
    (humanId: string, assignmentMode: AssignmentMode) => {
      if (segment.words.length === 0) return;
      const anchorWordId = getAssignmentAnchorWordId(segment);
      if (!anchorWordId) return;
      const scrollContainer =
        triggerRef.current?.closest<HTMLElement>(
          "[data-transcript-container]",
        ) ?? null;
      void preserveScrollPosition(scrollContainer, () =>
        assignTranscriptSpeaker({
          transcriptId,
          segmentKey: segment.key,
          humanId,
          anchorWordId,
          mode: assignmentMode,
          wordIds: getAssignmentWordIds(segment),
        }),
      )
        .then(() => {
          trackAnalyticsEvent("participant_assigned", {
            assignment_scope: assignmentMode,
            word_count: segment.words.length,
          });
          onAssigned?.(humanId);
          handleOpenChange(false);
        })
        .catch((error) => {
          console.error("[transcript] failed to assign speaker", error);
        });
    },
    [handleOpenChange, onAssigned, transcriptId, segment],
  );

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger asChild>
        <button
          ref={triggerRef}
          type="button"
          data-transcript-speaker-assign
          {...mergeStyleXProps(
            [styles.trigger, open && styles.underlined],
            className,
            { color },
          )}
        >
          {label}
        </button>
      </PopoverTrigger>
      <PopoverContent
        variant="app"
        side="right"
        align="start"
        sideOffset={8}
        collisionPadding={16}
        sx={styles.popover}
      >
        <SpeakerParticipantPicker
          sessionId={sessionId}
          onSelect={handleAssign}
        />
      </PopoverContent>
    </Popover>
  );
}

export function getAssignmentAnchorWordId(
  segment: Segment,
): string | undefined {
  const word = segment.words.find(
    (word) => typeof word.id === "string" && word.id.length > 0,
  );
  return typeof word?.id === "string" ? word.id : undefined;
}

export function getAssignmentWordIds(segment: Segment): string[] {
  return segment.words
    .map((word) => word.id)
    .filter(
      (wordId): wordId is string =>
        typeof wordId === "string" && wordId.length > 0,
    );
}

export type SpeakerParticipantOption = {
  id: string;
  name: string;
  email?: string;
  avatarDataUrl?: string;
  isSessionParticipant: boolean;
  isNew?: boolean;
  isCreateOption?: boolean;
};

export function buildSpeakerParticipantGroups({
  sessionParticipants,
  eventParticipants = [],
  contacts,
  query,
}: {
  sessionParticipants: SpeakerParticipantOption[];
  eventParticipants?: SpeakerParticipantOption[];
  contacts: SpeakerParticipantOption[];
  query: string;
}) {
  const normalizedQuery = query.trim().toLowerCase();
  const matches = (option: SpeakerParticipantOption) => {
    if (!normalizedQuery) {
      return true;
    }

    return [option.name, option.email ?? ""].some((value) =>
      value.toLowerCase().includes(normalizedQuery),
    );
  };

  const participantKeys = new Set<string>();
  const participantOptions = [...sessionParticipants, ...eventParticipants]
    .filter((option) => {
      const keys = getSpeakerParticipantDedupeKeys(option);
      if (keys.some((key) => participantKeys.has(key))) {
        return false;
      }

      keys.forEach((key) => participantKeys.add(key));
      return true;
    })
    .filter(matches);
  const matchingContacts = contacts
    .filter((option) =>
      getSpeakerParticipantDedupeKeys(option).every(
        (key) => !participantKeys.has(key),
      ),
    )
    .filter(matches);

  return [
    ...(participantOptions.length > 0
      ? [
          {
            title: "Participants",
            options: participantOptions,
          },
        ]
      : []),
    ...(matchingContacts.length > 0
      ? [
          {
            title: "People",
            options: matchingContacts,
          },
        ]
      : []),
  ];
}

export function buildCreateSpeakerParticipantOption({
  query,
  existingOptions,
}: {
  query: string;
  existingOptions: SpeakerParticipantOption[];
}): SpeakerParticipantOption | null {
  const name = query.trim();
  if (!name) {
    return null;
  }

  const normalizedName = name.toLowerCase();
  const alreadyExists = existingOptions.some((option) =>
    [option.name, option.email ?? ""].some(
      (value) => value.toLowerCase() === normalizedName,
    ),
  );
  if (alreadyExists) {
    return null;
  }

  return {
    id: "new",
    name,
    isSessionParticipant: false,
    isNew: true,
    isCreateOption: true,
  };
}

export function buildEventSpeakerParticipantOptions({
  eventParticipants,
  contacts,
}: {
  eventParticipants: EventParticipant[];
  contacts: SpeakerParticipantOption[];
}): SpeakerParticipantOption[] {
  const contactByEmail = new Map(
    contacts
      .filter((contact) => contact.email)
      .map((contact) => [contact.email!.toLowerCase(), contact]),
  );
  const contactByName = new Map(
    contacts.map((contact) => [contact.name.toLowerCase(), contact]),
  );

  return eventParticipants
    .map((participant, index): SpeakerParticipantOption | null => {
      const name = (participant.name ?? "").trim();
      const email = (participant.email ?? "").trim();
      if (!name && !email) {
        return null;
      }

      const contact = email
        ? contactByEmail.get(email.toLowerCase())
        : name
          ? contactByName.get(name.toLowerCase())
          : undefined;

      if (contact) {
        return {
          ...contact,
          name: name || contact.name,
          email: email || contact.email,
          isSessionParticipant: true,
        };
      }

      const pendingId = email ? `event:${email}` : `event:${name}:${index}`;

      return {
        id: pendingId,
        name: name || email,
        email: email || undefined,
        isSessionParticipant: true,
        isNew: true,
      };
    })
    .filter((option): option is SpeakerParticipantOption => option !== null);
}

export function SpeakerParticipantPicker({
  sessionId,
  onSelect,
  showAssignmentScope = true,
}: {
  sessionId: string | undefined;
  onSelect: (humanId: string, mode: AssignmentMode) => void | Promise<void>;
  showAssignmentScope?: boolean;
}) {
  const { t } = useLingui();
  const session = useSession(sessionId ?? "");
  const participantRecords = useSessionParticipants(sessionId ?? "");
  const humanRecords = useHumans();
  const attachedEventParticipants = useSessionEventParticipants(
    sessionId ?? "",
  );

  const [query, setQuery] = useState("");
  const [selectedOption, setSelectedOption] =
    useState<SpeakerParticipantOption | null>(null);
  const [applyToAllMatching, setApplyToAllMatching] = useState(true);
  const [assigning, setAssigning] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);

  const avatarByHumanId = useMemo(
    () =>
      new Map(
        humanRecords.map((human) => [human.id, human.avatarDataUrl] as const),
      ),
    [humanRecords],
  );
  const participants = useMemo(
    () =>
      participantRecords
        .map((participant): SpeakerParticipantOption | null => {
          if (!participant.humanId) return null;
          const name = participant.name.trim();
          const email = participant.email.trim();
          return {
            id: participant.humanId,
            name: name || email || t`Unknown`,
            email: email || undefined,
            avatarDataUrl:
              avatarByHumanId.get(participant.humanId) ?? undefined,
            isSessionParticipant: true,
          };
        })
        .filter((participant): participant is SpeakerParticipantOption =>
          Boolean(participant),
        ),
    [avatarByHumanId, participantRecords, t],
  );

  const contacts = useMemo(
    () =>
      humanRecords
        .map((human): SpeakerParticipantOption | null => {
          const name = human.name.trim();
          const email = human.email.trim();
          if (!name && !email) return null;

          return {
            id: human.id,
            name: name || email,
            email: email || undefined,
            avatarDataUrl: human.avatarDataUrl ?? undefined,
            isSessionParticipant: false,
          };
        })
        .filter((contact): contact is SpeakerParticipantOption =>
          Boolean(contact),
        ),
    [humanRecords],
  );

  const eventParticipants = useMemo(
    () =>
      buildEventSpeakerParticipantOptions({
        eventParticipants: attachedEventParticipants,
        contacts,
      }),
    [attachedEventParticipants, contacts],
  );

  const participantIds = useMemo(
    () => new Set(participants.map((participant) => participant.id)),
    [participants],
  );

  const groups = useMemo(
    () =>
      buildSpeakerParticipantGroups({
        sessionParticipants: participants,
        eventParticipants,
        contacts,
        query,
      }),
    [contacts, eventParticipants, participants, query],
  );

  const createOption = useMemo(
    () =>
      buildCreateSpeakerParticipantOption({
        query,
        existingOptions: [...participants, ...eventParticipants, ...contacts],
      }),
    [contacts, eventParticipants, participants, query],
  );
  const hasPeopleGroup = groups.some((group) => group.title === "People");

  const linkHumanToSession = useCallback(
    async (humanId: string) => {
      if (!sessionId || participantIds.has(humanId)) {
        return;
      }

      await addSessionParticipant(sessionId, humanId);
    },
    [participantIds, sessionId],
  );

  const handleSelect = useCallback((option: SpeakerParticipantOption) => {
    setSelectedOption(option);
  }, []);

  const getCurrentHumanId = useCallback(
    async (option: SpeakerParticipantOption) => {
      if (!option.isNew) {
        return option.id;
      }

      const email = option.email?.trim().toLowerCase();
      const name = option.name.trim().toLowerCase();
      const existingContact = email
        ? contacts.find(
            (contact) => contact.email?.trim().toLowerCase() === email,
          )
        : contacts.find(
            (contact) => contact.name.trim().toLowerCase() === name,
          );

      if (existingContact) return existingContact.id;
      if (!session?.user_id) return null;

      return createHuman({
        ownerUserId: session.user_id,
        name: option.name,
        email: option.email,
        entryPoint: "speaker_assignment",
      });
    },
    [contacts, session?.user_id],
  );

  const handleConfirm = useCallback(() => {
    if (!selectedOption) {
      return;
    }

    setAssigning(true);
    void getCurrentHumanId(selectedOption)
      .then(async (humanId) => {
        if (!humanId) return;
        await linkHumanToSession(humanId);
        await onSelect(
          humanId,
          showAssignmentScope && applyToAllMatching ? "all" : "segment",
        );
      })
      .catch((error) => {
        console.error("[transcript] failed to prepare speaker", error);
      })
      .finally(() => setAssigning(false));
  }, [
    applyToAllMatching,
    getCurrentHumanId,
    linkHumanToSession,
    onSelect,
    selectedOption,
    showAssignmentScope,
  ]);

  return (
    <div {...stylex.props(styles.picker)}>
      <AppFloatingPanel sx={styles.panel}>
        <div {...stylex.props(styles.searchFrame)}>
          <div {...stylex.props(styles.searchRow)}>
            <MagnifyingGlass size={16} {...stylex.props(styles.searchIcon)} />
            <input
              ref={searchInputRef}
              autoFocus
              type="search"
              {...stylex.props(styles.searchInput)}
              placeholder={t`Select or type to add speaker`}
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setSelectedOption(null);
              }}
            />
          </div>
        </div>
        <div {...stylex.props(styles.options)}>
          {groups.map((group) => (
            <div key={group.title}>
              <div {...stylex.props(styles.groupHeading)}>
                {group.title === "Participants" ? (
                  <Trans>Participants</Trans>
                ) : (
                  <Trans>People</Trans>
                )}
              </div>
              {group.options.map((option) => (
                <ParticipantOptionButton
                  key={option.id}
                  option={option}
                  selected={selectedOption === option}
                  onSelect={handleSelect}
                />
              ))}
            </div>
          ))}

          {createOption && (
            <div>
              {!hasPeopleGroup && (
                <div {...stylex.props(styles.groupHeading)}>
                  <Trans>People</Trans>
                </div>
              )}
              <ParticipantOptionButton
                option={createOption}
                selected={selectedOption === createOption}
                onSelect={handleSelect}
              />
            </div>
          )}

          {!createOption && groups.length === 0 && (
            <p {...stylex.props(styles.empty)}>
              {query.trim() ? (
                <Trans>No matching people</Trans>
              ) : (
                <Trans>No people</Trans>
              )}
            </p>
          )}

          {!query.trim() && (
            <button
              type="button"
              {...stylex.props(styles.createButton)}
              onClick={() => searchInputRef.current?.focus()}
            >
              <Plus {...stylex.props(styles.icon)} />
              <Trans>Create new speaker</Trans>
            </button>
          )}
        </div>
      </AppFloatingPanel>
      <div {...stylex.props(styles.footer)}>
        {showAssignmentScope && (
          <label {...stylex.props(styles.scopeLabel)}>
            <Checkbox
              checked={applyToAllMatching}
              onCheckedChange={(value) => setApplyToAllMatching(value === true)}
            />
            <span {...stylex.props(styles.scopeText)}>
              <Trans>Apply to all</Trans>
            </span>
          </label>
        )}
        <button
          type="button"
          {...stylex.props(styles.confirmButton)}
          disabled={!selectedOption || assigning}
          onClick={handleConfirm}
        >
          <Trans>Confirm</Trans>
        </button>
      </div>
    </div>
  );
}

function getSpeakerParticipantDedupeKeys(
  option: SpeakerParticipantOption,
): string[] {
  return [
    `id:${option.id}`,
    option.email ? `email:${option.email.toLowerCase()}` : null,
  ].filter((key): key is string => key !== null);
}

function ParticipantOptionButton({
  option,
  selected,
  onSelect,
}: {
  option: SpeakerParticipantOption;
  selected: boolean;
  onSelect: (option: SpeakerParticipantOption) => void;
}) {
  const { t } = useLingui();
  return (
    <button
      type="button"
      aria-pressed={selected}
      {...stylex.props(
        styles.optionButton,
        selected ? styles.optionSelected : styles.optionIdle,
      )}
      onClick={() => onSelect(option)}
    >
      {option.isCreateOption ? (
        <span {...stylex.props(styles.createAvatar)}>
          <Plus {...stylex.props(styles.smallIcon)} aria-hidden="true" />
        </span>
      ) : option.avatarDataUrl ? (
        <ContactImage src={option.avatarDataUrl} size={28} />
      ) : (
        <ContactFacehash
          name={option.name || option.email || option.id}
          size={28}
        />
      )}
      <span {...stylex.props(styles.optionText)}>
        <span {...stylex.props(styles.optionName)}>
          {option.isCreateOption ? t`Add "${option.name}"` : option.name}
        </span>
        {option.email && (
          <span {...stylex.props(styles.optionEmail)}>{option.email}</span>
        )}
      </span>
    </button>
  );
}

const styles = stylex.create({
  confirmButton: {
    backgroundColor: {
      default: colors.primary,
      ":hover": `color-mix(in srgb, ${colors.primary} 90%, transparent)`,
    },
    borderRadius: radii.full,
    color: colors.primaryForeground,
    fontSize: "0.75rem",
    fontWeight: 500,
    height: "2rem",
    opacity: {
      default: 1,
      ":disabled": 0.5,
    },
    paddingInline: "0.75rem",
    pointerEvents: {
      default: "auto",
      ":disabled": "none",
    },
  },
  createAvatar: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    display: "flex",
    flexShrink: 0,
    height: "1.75rem",
    justifyContent: "center",
    width: "1.75rem",
  },
  createButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    width: "100%",
  },
  empty: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
  },
  footer: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "flex-end",
    paddingBottom: "0.25rem",
    paddingLeft: "0.5rem",
    paddingTop: "0.25rem",
  },
  groupHeading: {
    color: colors.mutedForeground,
    fontSize: "0.6875rem",
    fontWeight: 500,
    paddingBottom: "0.25rem",
    paddingLeft: "0.75rem",
    paddingRight: "0.75rem",
    paddingTop: "0.5rem",
    textTransform: "uppercase",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  optionButton: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.75rem",
    textAlign: "left",
    width: "100%",
  },
  optionEmail: {
    color: colors.mutedForeground,
    display: "block",
    fontSize: "0.75rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  optionIdle: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
  },
  optionName: {
    display: "block",
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  optionSelected: {
    backgroundColor: colors.accent,
    color: colors.accentForeground,
  },
  options: {
    flex: "1",
    minHeight: 0,
    overflow: "auto",
    paddingBlock: "0.25rem",
  },
  optionText: {
    flex: "1",
    minWidth: 0,
  },
  panel: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    minHeight: 0,
    overflow: "hidden",
  },
  picker: {
    display: "flex",
    flexDirection: "column",
    gap: "0.25rem",
    maxHeight:
      "min(var(--radix-popover-content-available-height, calc(100vh - 1rem)), 28rem)",
    overflow: "hidden",
  },
  popover: {
    maxHeight: "min(var(--radix-popover-content-available-height), 28rem)",
    width: "20rem",
  },
  scopeLabel: {
    alignItems: "center",
    cursor: "pointer",
    display: "flex",
    flex: "1",
    gap: "0.5rem",
    minWidth: 0,
  },
  scopeText: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    whiteSpace: "nowrap",
  },
  searchFrame: {
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    paddingBlock: "0.25rem",
  },
  searchIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
  },
  searchInput: {
    backgroundColor: "transparent",
    color: {
      default: null,
      "::placeholder": colors.mutedForeground,
    },
    flex: "1",
    fontSize: "0.875rem",
    minWidth: 0,
    outline: "none",
  },
  searchRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    height: "2rem",
    paddingInline: "0.75rem",
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  trigger: {
    borderRadius: radii.full,
    cursor: "pointer",
    marginBlock: "-0.125rem",
    paddingBottom: "0.125rem",
    paddingRight: "0.5rem",
    paddingTop: "0.125rem",
    textDecorationLine: {
      default: "none",
      ":focus-visible": "underline",
      ":hover": "underline",
    },
    textUnderlineOffset: "2px",
  },
  underlined: {
    textDecorationLine: "underline",
  },
});

export { styles as speakerAssignStyles };
