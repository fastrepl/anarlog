export function configureCenteredPlayback(
  media: HTMLMediaElement,
): AudioContext | null {
  const gainNode = (
    media as HTMLMediaElement & { getGainNode?: () => GainNode }
  ).getGainNode?.();

  if (!gainNode) {
    return null;
  }

  gainNode.channelCount = 1;
  gainNode.channelCountMode = "explicit";
  gainNode.channelInterpretation = "speakers";

  const context = gainNode.context;
  return "resume" in context && "close" in context
    ? (context as AudioContext)
    : null;
}
