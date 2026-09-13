import type {
  SpeakerContext,
  SpeakerContextInterval,
} from "@anlg/plugin-transcription";

export const EMPTY_SPEAKER_CONTEXT: SpeakerContext = { intervals: [] };

export function parseSpeakerContext(
  value: string | null | undefined,
): SpeakerContext {
  try {
    const parsed = JSON.parse(value ?? "null");
    if (!parsed || !Array.isArray(parsed.intervals))
      return EMPTY_SPEAKER_CONTEXT;
    return {
      intervals: parsed.intervals.filter(
        (item: SpeakerContextInterval) =>
          item &&
          Number.isSafeInteger(item.start_ms) &&
          Number.isSafeInteger(item.end_ms) &&
          item.start_ms < item.end_ms &&
          typeof item.active_call === "boolean" &&
          typeof item.calendar_call === "boolean" &&
          typeof item.shared_microphone === "boolean" &&
          (item.mic_isolated === null ||
            typeof item.mic_isolated === "boolean") &&
          typeof item.title === "string" &&
          Array.isArray(item.self_names) &&
          item.self_names.every((name) => typeof name === "string") &&
          Array.isArray(item.participants) &&
          item.participants.every(
            (person) =>
              person &&
              typeof person.human_id === "string" &&
              typeof person.name === "string",
          ),
      ),
    };
  } catch {
    return EMPTY_SPEAKER_CONTEXT;
  }
}

export function appendSpeakerObservation(
  context: SpeakerContext,
  observation: SpeakerContextInterval,
): SpeakerContext {
  const intervals = context.intervals.map((interval) => ({ ...interval }));
  const last = intervals[intervals.length - 1];
  if (last && observation.start_ms < last.start_ms) return context;
  if (last) {
    const facts = ({
      start_ms: _start,
      end_ms: _end,
      ...rest
    }: SpeakerContextInterval) => rest;
    if (
      last.end_ms >= observation.start_ms &&
      JSON.stringify(facts(last)) === JSON.stringify(facts(observation))
    ) {
      last.end_ms = observation.end_ms;
      return { intervals };
    }
    last.end_ms = Math.min(last.end_ms, observation.start_ms);
  }
  intervals.push(observation);
  return {
    intervals: intervals.filter(
      (interval) => interval.end_ms > interval.start_ms,
    ),
  };
}

export function closeSpeakerContext(
  context: SpeakerContext,
  at: number,
): SpeakerContext {
  return {
    intervals: context.intervals
      .map((interval) => ({
        ...interval,
        end_ms: Math.min(interval.end_ms, at),
      }))
      .filter((interval) => interval.end_ms > interval.start_ms),
  };
}

export function isSharedMicrophone(name: string | null): boolean {
  return /speakerphone|conference|meeting.?room|polycom|jabra speak|yeti|room.?mic|loopback|blackhole/i.test(
    name ?? "",
  );
}

export function isPersonalMicrophone(name: string | null): boolean {
  return (
    !isSharedMicrophone(name) &&
    /airpods|headset|earbuds|earphones|headphones/i.test(name ?? "")
  );
}
