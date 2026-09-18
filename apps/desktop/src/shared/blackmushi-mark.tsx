import { useId } from "react";

/** The BlackMushi mark: a snail whose shell doubles as a receiver horn.
 * The spiral is punched out with a mask rather than painted, so the mark
 * carries whatever is behind it and works on light and dark surfaces alike.
 *
 * The viewBox is the mark's ink box squared and padded: x 3..62.2 (the shell
 * disc reaches x=3, further left than the body does) and y 9..56.75. */
export function BlackMushiMark({ className }: { className?: string }) {
  // BrandLoadingView stacks two copies for the shimmer sweep, so the mask id
  // has to be per-instance or the second copy would reference the first mask.
  const maskId = useId();

  return (
    <svg
      viewBox="2 2.28 61.2 61.2"
      fill="none"
      className={className}
      aria-hidden="true"
    >
      <mask
        id={maskId}
        maskUnits="userSpaceOnUse"
        x="0"
        y="0"
        width="64"
        height="64"
      >
        <rect width="64" height="64" fill="#fff" />
        <path
          d="M40 32 A 14 14 0 0 1 12 32 A 10 10 0 0 1 32 32 A 5.5 5.5 0 0 1 21 32"
          fill="none"
          stroke="#000"
          strokeWidth="6"
          strokeLinecap="round"
        />
      </mask>
      <g fill="currentColor" stroke="currentColor">
        <path
          d="M14 52 L38 52 C47 52 50 47 50 35"
          fill="none"
          strokeWidth="9.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <path d="M51 34 L57 23" fill="none" strokeWidth="4.5" strokeLinecap="round" />
        <path d="M49 32 L46 16" fill="none" strokeWidth="3.8" strokeLinecap="round" />
        <circle cx="58" cy="20" r="4.2" stroke="none" />
        <circle cx="45" cy="13" r="3.5" stroke="none" />
        <circle cx="26" cy="32" r="23" stroke="none" mask={`url(#${maskId})`} />
      </g>
    </svg>
  );
}
