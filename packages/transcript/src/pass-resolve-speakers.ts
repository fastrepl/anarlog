import type {
  NormalizedWord,
  ResolvedWordFrame,
  SegmentWord,
  SpeakerIdentity,
  SpeakerState,
} from "./shared";

type SpeakerStateSnapshot = Pick<
  SpeakerState,
  | "completeChannels"
  | "humanIdByChannel"
  | "humanIdBySpeakerIndex"
  | "lastSpeakerByChannel"
>;

type IdentityRuleArgs = {
  assignment?: SpeakerIdentity;
  snapshot: SpeakerStateSnapshot;
  word: SegmentWord;
};

type IdentityRule = (
  identity: SpeakerIdentity,
  args: IdentityRuleArgs,
) => SpeakerIdentity;

export function resolveIdentities(
  words: NormalizedWord[],
  speakerState: SpeakerState,
): ResolvedWordFrame[] {
  return words.map((word, index) => {
    const assignment = speakerState.assignmentByWordIndex.get(index);
    const identity = applyIdentityRules(word, assignment, speakerState);
    rememberIdentity(word, assignment, identity, speakerState);

    return { word, identity };
  });
}

function applyIdentityRules(
  word: SegmentWord,
  assignment: SpeakerIdentity | undefined,
  snapshot: SpeakerStateSnapshot,
): SpeakerIdentity {
  const rules: IdentityRule[] = [
    applyExplicitAssignment,
    applySpeakerIndexHumanId,
    applyChannelHumanId,
    carryPartialIdentityForward,
  ];

  const args: IdentityRuleArgs = {
    assignment,
    snapshot,
    word,
  };

  return rules.reduce(
    (identity, rule) => rule(identity, args),
    {} as SpeakerIdentity,
  );
}

function rememberIdentity(
  word: SegmentWord,
  assignment: SpeakerIdentity | undefined,
  identity: SpeakerIdentity,
  state: SpeakerState,
): void {
  const hasExplicitAssignment =
    assignment !== undefined &&
    (assignment.speaker_index !== undefined ||
      assignment.human_id !== undefined);

  if (identity.speaker_index !== undefined && identity.human_id !== undefined) {
    state.humanIdBySpeakerIndex.set(identity.speaker_index, identity.human_id);
  }

  if (
    state.completeChannels.has(word.channel) &&
    identity.human_id !== undefined &&
    identity.speaker_index === undefined
  ) {
    state.humanIdByChannel.set(word.channel, identity.human_id);
  }

  if (
    !word.isFinal ||
    identity.speaker_index !== undefined ||
    hasExplicitAssignment
  ) {
    if (
      identity.speaker_index !== undefined ||
      identity.human_id !== undefined
    ) {
      state.lastSpeakerByChannel.set(word.channel, { ...identity });
    }
  }
}

const applyExplicitAssignment: IdentityRule = (identity, { assignment }) => {
  if (!assignment) return identity;
  return {
    ...identity,
    ...(assignment.speaker_index !== undefined && {
      speaker_index: assignment.speaker_index,
    }),
    ...(assignment.human_id !== undefined && { human_id: assignment.human_id }),
  };
};

const applySpeakerIndexHumanId: IdentityRule = (identity, { snapshot }) => {
  if (identity.speaker_index === undefined || identity.human_id !== undefined) {
    return identity;
  }

  const humanId = snapshot.humanIdBySpeakerIndex.get(identity.speaker_index);
  if (humanId !== undefined) {
    return { ...identity, human_id: humanId };
  }

  return identity;
};

const applyChannelHumanId: IdentityRule = (identity, { snapshot, word }) => {
  if (identity.human_id !== undefined) {
    return identity;
  }

  if (!snapshot.completeChannels.has(word.channel)) {
    return identity;
  }

  const humanId = snapshot.humanIdByChannel.get(word.channel);
  if (humanId !== undefined) {
    return { ...identity, human_id: humanId };
  }

  return identity;
};

const carryPartialIdentityForward: IdentityRule = (
  identity,
  { snapshot, word },
) => {
  if (
    word.isFinal ||
    (identity.speaker_index !== undefined && identity.human_id !== undefined)
  ) {
    return identity;
  }

  const last = snapshot.lastSpeakerByChannel.get(word.channel);
  if (!last) return identity;

  return {
    ...identity,
    ...(identity.speaker_index === undefined &&
      last.speaker_index !== undefined && {
        speaker_index: last.speaker_index,
      }),
    ...(identity.human_id === undefined &&
      last.human_id !== undefined && { human_id: last.human_id }),
  };
};
