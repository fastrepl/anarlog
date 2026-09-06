export function scrollVisibility(
  previous: { anchor: number; hidden: boolean },
  offset: number,
  maxOffset: number,
) {
  "worklet";
  const position = Math.max(0, Math.min(offset, Math.max(0, maxOffset)));
  if (position <= 0) return { anchor: 0, hidden: false };

  const anchor = previous.hidden
    ? Math.max(previous.anchor, position)
    : Math.min(previous.anchor, position);
  const distance = position - anchor;
  const hidden = previous.hidden ? distance > -12 : distance >= 12;

  return {
    anchor: hidden === previous.hidden ? anchor : position,
    hidden,
  };
}
