import { HStack, Image, Spacer, Text, VStack } from "@expo/ui/swift-ui";
import {
  activityBackgroundTint,
  font,
  foregroundStyle,
  frame,
  monospacedDigit,
  padding,
  symbolEffect,
} from "@expo/ui/swift-ui/modifiers";
import {
  createLiveActivity,
  type LiveActivity,
  type LiveActivityEnvironment,
} from "expo-widgets";

type MeetingRecordingActivityProps = {
  startedAtEpochMs: number;
};

const MeetingRecordingActivity = (
  props: MeetingRecordingActivityProps,
  _environment: LiveActivityEnvironment,
) => {
  "widget";
  "use no memo";

  const ACTIVITY_LIFETIME_MS = 8 * 60 * 60 * 1_000;
  const ACCENT_COLOR = "#FF453A";
  const PRIMARY_COLOR = "#FFFFFF";
  const SECONDARY_COLOR = "#FFFFFFA8";
  const startedAt = new Date(props.startedAtEpochMs);
  const activityEndsAt = new Date(
    props.startedAtEpochMs + ACTIVITY_LIFETIME_MS,
  );

  const Waveform = ({ size }: { size: number }) => {
    "use no memo";

    return (
      <Image
        systemName="waveform"
        size={size}
        color={PRIMARY_COLOR}
        modifiers={[
          symbolEffect(
            {
              effect: "variableColor",
              fillStyle: "iterative",
              playbackStyle: "reversing",
            },
            { options: { repeat: "continuous", speed: 0.8 } },
          ),
        ]}
      />
    );
  };

  const ElapsedTime = ({ width }: { width: number }) => {
    "use no memo";

    return (
      <Text
        timerInterval={{ lower: startedAt, upper: activityEndsAt }}
        countsDown={false}
        modifiers={[
          font({ size: 14, weight: "medium", design: "rounded" }),
          monospacedDigit(),
          foregroundStyle(PRIMARY_COLOR),
          frame({ width, alignment: "trailing" }),
        ]}
      />
    );
  };

  return {
    banner: (
      <HStack
        spacing={12}
        modifiers={[
          activityBackgroundTint("#1C1A18"),
          frame({ maxWidth: Infinity }),
          padding({ all: 16 }),
        ]}
      >
        <Image systemName="circle.fill" size={10} color={ACCENT_COLOR} />
        <VStack alignment="leading" spacing={3}>
          <Text
            modifiers={[
              font({ size: 16, weight: "semibold" }),
              foregroundStyle(PRIMARY_COLOR),
            ]}
          >
            Recording meeting
          </Text>
          <HStack spacing={6}>
            <Waveform size={15} />
            <Text
              modifiers={[font({ size: 13 }), foregroundStyle(SECONDARY_COLOR)]}
            >
              Anarlog is listening
            </Text>
          </HStack>
        </VStack>
        <Spacer />
        <ElapsedTime width={58} />
      </HStack>
    ),
    compactLeading: (
      <HStack spacing={5} modifiers={[padding({ leading: 4 })]}>
        <Image systemName="circle.fill" size={7} color={ACCENT_COLOR} />
        <Waveform size={15} />
      </HStack>
    ),
    compactTrailing: <ElapsedTime width={50} />,
    minimal: <Waveform size={16} />,
    expandedLeading: (
      <HStack spacing={7} modifiers={[padding({ leading: 6 })]}>
        <Image systemName="circle.fill" size={8} color={ACCENT_COLOR} />
        <Text
          modifiers={[
            font({ size: 14, weight: "semibold" }),
            foregroundStyle(PRIMARY_COLOR),
          ]}
        >
          Anarlog
        </Text>
      </HStack>
    ),
    expandedTrailing: (
      <HStack modifiers={[padding({ trailing: 6 })]}>
        <ElapsedTime width={58} />
      </HStack>
    ),
    expandedBottom: (
      <HStack spacing={8} modifiers={[padding({ top: 4, horizontal: 6 })]}>
        <Waveform size={18} />
        <Text
          modifiers={[
            font({ size: 14, weight: "medium" }),
            foregroundStyle(SECONDARY_COLOR),
          ]}
        >
          Recording meeting
        </Text>
      </HStack>
    ),
  };
};

type ActivityFactory = ReturnType<
  typeof createLiveActivity<MeetingRecordingActivityProps>
>;

let activityFactory: ActivityFactory | null = null;
let activeSessionId: string | null = null;
let activeActivity: LiveActivity<MeetingRecordingActivityProps> | null = null;
let operationQueue = Promise.resolve();

function enqueue(operation: () => Promise<void>): Promise<void> {
  const next = operationQueue.then(operation, operation);
  operationQueue = next.catch(() => {});
  return next;
}

function getActiveActivities(): LiveActivity<MeetingRecordingActivityProps>[] {
  const activities = getActivityFactory().getInstances();
  if (
    activeActivity &&
    !activities.some((activity) => activity.getId() === activeActivity?.getId())
  ) {
    activities.push(activeActivity);
  }
  return activities;
}

function getActivityFactory(): ActivityFactory {
  activityFactory ??= createLiveActivity<MeetingRecordingActivityProps>(
    "MeetingRecordingActivity",
    MeetingRecordingActivity,
  );
  return activityFactory;
}

async function clearActiveActivities(): Promise<void> {
  const activities = getActiveActivities();
  activeActivity = null;
  activeSessionId = null;
  const results = await Promise.allSettled(
    activities.map((activity) => activity.end("immediate")),
  );
  const rejected = results.find((result) => result.status === "rejected");
  if (rejected?.status === "rejected") throw rejected.reason;
}

export function clearStaleMeetingRecordingActivities(): Promise<void> {
  return enqueue(clearActiveActivities);
}

export function startMeetingRecordingActivity(
  sessionId: string,
): Promise<void> {
  return enqueue(async () => {
    if (activeSessionId === sessionId && activeActivity) return;

    await Promise.allSettled(
      getActiveActivities().map((activity) => activity.end("immediate")),
    );

    activeActivity = getActivityFactory().start(
      { startedAtEpochMs: Date.now() },
      `anarlog://note/${sessionId}`,
    );
    activeSessionId = sessionId;
  });
}

export function endMeetingRecordingActivity(sessionId: string): Promise<void> {
  return enqueue(async () => {
    if (activeSessionId && activeSessionId !== sessionId) return;
    await clearActiveActivities();
  });
}
