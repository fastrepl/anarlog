import { cn } from "@hypr/utils";

export type SocialPlatform = "reddit" | "linkedin" | "twitter";

export interface SocialCardProps {
  platform: SocialPlatform;
  author: string;
  body: string;
  url: string;
  className?: string;
  username?: string;
  subreddit?: string;
  role?: string;
  company?: string;
  avatar?: string;
}

const WARM_PALETTES = [
  ["#d6bcfa", "#6b21a8"],
  ["#fecaca", "#991b1b"],
  ["#bfdbfe", "#1e40af"],
  ["#bbf7d0", "#166534"],
  ["#fde68a", "#92400e"],
  ["#fbcfe8", "#9d174d"],
  ["#c7d2fe", "#3730a3"],
  ["#fed7aa", "#9a3412"],
];

function getInitials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0].toUpperCase())
    .join("");
}

function hashString(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash);
  }
  return Math.abs(hash);
}

function AuthorAvatar({ author, avatar }: { author: string; avatar?: string }) {
  if (avatar) {
    return (
      <img
        src={avatar}
        alt={author}
        className="size-9 rounded-full object-cover"
      />
    );
  }

  const palette = WARM_PALETTES[hashString(author) % WARM_PALETTES.length];
  const initials = getInitials(author);

  return (
    <div
      className="flex size-9 shrink-0 items-center justify-center rounded-full text-sm font-medium"
      style={{ backgroundColor: palette[0], color: palette[1] }}
    >
      {initials}
    </div>
  );
}

const platformConfig = {
  reddit: {
    iconColor: "text-orange-600",
    iconPath:
      "M12 0A12 12 0 0 0 0 12a12 12 0 0 0 12 12 12 12 0 0 0 12-12A12 12 0 0 0 12 0zm5.01 4.744c.688 0 1.25.561 1.25 1.249a1.25 1.25 0 0 1-2.498.056l-2.597-.547-.8 3.747c1.824.07 3.48.632 4.674 1.488.308-.309.73-.491 1.207-.491.968 0 1.754.786 1.754 1.754 0 .716-.435 1.333-1.01 1.614a3.111 3.111 0 0 1 .042.52c0 2.694-3.13 4.87-7.004 4.87-3.874 0-7.004-2.176-7.004-4.87 0-.183.015-.366.043-.534A1.748 1.748 0 0 1 4.028 12c0-.968.786-1.754 1.754-1.754.463 0 .898.196 1.207.49 1.207-.883 2.878-1.43 4.744-1.487l.885-4.182a.342.342 0 0 1 .14-.197.35.35 0 0 1 .238-.042l2.906.617a1.214 1.214 0 0 1 1.108-.701zM9.25 12C8.561 12 8 12.562 8 13.25c0 .687.561 1.248 1.25 1.248.687 0 1.248-.561 1.248-1.249 0-.688-.561-1.249-1.249-1.249zm5.5 0c-.687 0-1.248.561-1.248 1.25 0 .687.561 1.248 1.249 1.248.688 0 1.249-.561 1.249-1.249 0-.687-.562-1.249-1.25-1.249zm-5.466 3.99a.327.327 0 0 0-.231.094.33.33 0 0 0 0 .463c.842.842 2.484.913 2.961.913.477 0 2.105-.056 2.961-.913a.361.361 0 0 0 .029-.463.33.33 0 0 0-.464 0c-.547.533-1.684.73-2.512.73-.828 0-1.979-.196-2.512-.73a.326.326 0 0 0-.232-.095z",
  },
  linkedin: {
    iconColor: "text-blue-700",
    iconPath:
      "M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433c-1.144 0-2.063-.926-2.063-2.065 0-1.138.92-2.063 2.063-2.063 1.14 0 2.064.925 2.064 2.063 0 1.139-.925 2.065-2.064 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z",
  },
  twitter: {
    iconColor: "text-fg",
    iconPath:
      "M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z",
  },
};

function getSubtitle(props: SocialCardProps): string {
  if (props.platform === "reddit" && props.subreddit)
    return `r/${props.subreddit}`;
  if (props.platform === "linkedin" && props.role && props.company)
    return `${props.role} at ${props.company}`;
  if (props.platform === "twitter" && props.username)
    return `@${props.username}`;
  return "";
}

const TAIL_R = 12;
const tailMask = `radial-gradient(${TAIL_R}px at 100% 0, #0000 98%, #000 101%)`;

const HIGHLIGHT_RE =
  /(@tryhyprnote|@getcharnotes|Hyprnote|Hypernote|hyprnote)/gi;
const HIGHLIGHT_TEST =
  /^(@tryhyprnote|@getcharnotes|Hyprnote|Hypernote|hyprnote)$/i;

function HighlightedBody({ text }: { text: string }) {
  const parts = text.split(HIGHLIGHT_RE);
  return (
    <>
      {parts.map((part, i) =>
        HIGHLIGHT_TEST.test(part) ? (
          <span key={i} className="text-fg font-semibold">
            {part}
          </span>
        ) : (
          part
        ),
      )}
    </>
  );
}

export function SocialCard(props: SocialCardProps) {
  const { platform, author, body, url, className, avatar } = props;
  const config = platformConfig[platform];
  const subtitle = getSubtitle(props);

  return (
    <div className={cn(["flex flex-col gap-3", className])}>
      <div
        className="relative mb-1"
        style={{
          filter: "drop-shadow(0 1px 2px rgba(0,0,0,0.06))",
        }}
      >
        <div className="bg-surface rounded-2xl rounded-bl-none px-5 py-4">
          <p className="text-fg-muted text-base leading-relaxed md:line-clamp-[20]">
            <HighlightedBody text={body} />
          </p>
        </div>
        <div
          className="bg-surface absolute left-0"
          style={{
            width: TAIL_R,
            height: TAIL_R,
            bottom: -TAIL_R,
            borderRadius: `0 0 0 ${TAIL_R}px`,
            mask: tailMask,
            WebkitMask: tailMask,
          }}
        />
      </div>
      <div className="flex items-center gap-3 pl-1">
        <a
          href={url}
          target="_blank"
          rel="noopener noreferrer"
          className="flex shrink-0 flex-row items-center gap-3 transition-opacity hover:opacity-70"
        >
          <AuthorAvatar author={author} avatar={avatar} />
          <div className="flex flex-col">
            <div className="flex items-center gap-1.5">
              <p className="text-fg text-base leading-relaxed font-medium">
                {author}
              </p>
              <svg
                className={cn(["size-3.5", config.iconColor])}
                fill="currentColor"
                viewBox="0 0 24 24"
              >
                <path d={config.iconPath} />
              </svg>
            </div>
            {subtitle && (
              <span className="text-fg-subtle text-xs">{subtitle}</span>
            )}
          </div>
        </a>
      </div>
    </div>
  );
}
