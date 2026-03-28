import { Icon } from "@iconify-icon/react";

import { cn } from "@hypr/utils";

import {
  GITHUB_LAST_SEEN_FORKS,
  GITHUB_LAST_SEEN_STARS,
  GITHUB_ORG_REPO,
  useGitHubStats,
} from "../queries";

const CURATED_PROFILES = [
  {
    username: "tobi",
    avatar: "https://avatars.githubusercontent.com/u/347?v=4",
  },
  {
    username: "DidierRLopes",
    avatar: "https://avatars.githubusercontent.com/u/25267873?v=4",
  },
  {
    username: "FelixMalfait",
    avatar: "https://avatars.githubusercontent.com/u/6399865?v=4",
  },
  {
    username: "jeremyfowers",
    avatar: "https://avatars.githubusercontent.com/u/80718789?v=4",
  },
  {
    username: "micheleriva",
    avatar: "https://avatars.githubusercontent.com/u/14977595?v=4",
  },
  {
    username: "thomwolf",
    avatar: "https://avatars.githubusercontent.com/u/7353373?v=4",
  },
  {
    username: "brodock",
    avatar: "https://avatars.githubusercontent.com/u/20575?v=4",
  },
  {
    username: "anthonycorletti",
    avatar: "https://avatars.githubusercontent.com/u/3477132?v=4",
  },
  {
    username: "followingell",
    avatar: "https://avatars.githubusercontent.com/u/5324956?v=4",
  },
  {
    username: "mbanzi",
    avatar: "https://avatars.githubusercontent.com/u/405127?v=4",
  },
  {
    username: "kevinxh",
    avatar: "https://avatars.githubusercontent.com/u/10948652?v=4",
  },
  {
    username: "gregnr",
    avatar: "https://avatars.githubusercontent.com/u/4133076?v=4",
  },
  {
    username: "JoeDo",
    avatar: "https://avatars.githubusercontent.com/u/775702?v=4",
  },
  {
    username: "toby",
    avatar: "https://avatars.githubusercontent.com/u/83556?v=4",
  },
  {
    username: "patrick91",
    avatar: "https://avatars.githubusercontent.com/u/667029?v=4",
  },
  {
    username: "timrogers",
    avatar: "https://avatars.githubusercontent.com/u/116134?v=4",
  },
  {
    username: "freeqaz",
    avatar: "https://avatars.githubusercontent.com/u/4573221?v=4",
  },
  {
    username: "robertefreeman",
    avatar: "https://avatars.githubusercontent.com/u/6842762?v=4",
  },
  {
    username: "mriley",
    avatar: "https://avatars.githubusercontent.com/u/28009?v=4",
  },
  {
    username: "pmdartus",
    avatar: "https://avatars.githubusercontent.com/u/2567083?v=4",
  },
  {
    username: "ezekg",
    avatar: "https://avatars.githubusercontent.com/u/6979737?v=4",
  },
  {
    username: "Jonathanvwersch",
    avatar: "https://avatars.githubusercontent.com/u/38623677?v=4",
  },
  {
    username: "thewh1teagle",
    avatar: "https://avatars.githubusercontent.com/u/61390950?v=4",
  },
  {
    username: "dguido",
    avatar: "https://avatars.githubusercontent.com/u/294844?v=4",
  },
  {
    username: "calvinfo",
    avatar: "https://avatars.githubusercontent.com/u/487539?v=4",
  },
  {
    username: "velyan",
    avatar: "https://avatars.githubusercontent.com/u/1313779?v=4",
  },
  {
    username: "mfts",
    avatar: "https://avatars.githubusercontent.com/u/4049052?v=4",
  },
  {
    username: "devgony",
    avatar: "https://avatars.githubusercontent.com/u/51254761?v=4",
  },
  {
    username: "bartoszgrabski",
    avatar: "https://avatars.githubusercontent.com/u/5851315?v=4",
  },
  {
    username: "mpazik",
    avatar: "https://avatars.githubusercontent.com/u/4086126?v=4",
  },
  {
    username: "Necromenta",
    avatar: "https://avatars.githubusercontent.com/u/95664440?v=4",
  },
  {
    username: "jonpage0",
    avatar: "https://avatars.githubusercontent.com/u/48391075?v=4",
  },
  {
    username: "ralder",
    avatar: "https://avatars.githubusercontent.com/u/10889830?v=4",
  },
  {
    username: "mateusrevoredo",
    avatar: "https://avatars.githubusercontent.com/u/1175432?v=4",
  },
  {
    username: "annieappflowy",
    avatar: "https://avatars.githubusercontent.com/u/12026239?v=4",
  },
  {
    username: "carllippert",
    avatar: "https://avatars.githubusercontent.com/u/16457876?v=4",
  },
  {
    username: "avneetsb",
    avatar: "https://avatars.githubusercontent.com/u/5681972?v=4",
  },
  {
    username: "anrath",
    avatar: "https://avatars.githubusercontent.com/u/62771105?v=4",
  },
  {
    username: "srikanta30",
    avatar: "https://avatars.githubusercontent.com/u/28688901?v=4",
  },
  {
    username: "allisoneer",
    avatar: "https://avatars.githubusercontent.com/u/20910163?v=4",
  },
  {
    username: "kebot",
    avatar: "https://avatars.githubusercontent.com/u/289392?v=4",
  },
  {
    username: "daevaorn",
    avatar: "https://avatars.githubusercontent.com/u/37366?v=4",
  },
  {
    username: "rdt712",
    avatar: "https://avatars.githubusercontent.com/u/13369991?v=4",
  },
  {
    username: "olabrainy",
    avatar: "https://avatars.githubusercontent.com/u/28204401?v=4",
  },
  {
    username: "aaronrau",
    avatar: "https://avatars.githubusercontent.com/u/207538?v=4",
  },
  {
    username: "jhbao",
    avatar: "https://avatars.githubusercontent.com/u/1714002?v=4",
  },
  {
    username: "dbkegley",
    avatar: "https://avatars.githubusercontent.com/u/5727001?v=4",
  },
  {
    username: "chrismalek",
    avatar: "https://avatars.githubusercontent.com/u/9403?v=4",
  },
  {
    username: "KlimDos",
    avatar: "https://avatars.githubusercontent.com/u/17221993?v=4",
  },
  {
    username: "maximilianmessing",
    avatar: "https://avatars.githubusercontent.com/u/7516094?v=4",
  },
  {
    username: "levysantanna",
    avatar: "https://avatars.githubusercontent.com/u/1235238?v=4",
  },
  {
    username: "falltodis",
    avatar: "https://avatars.githubusercontent.com/u/7006864?v=4",
  },
];

function StatBadge({
  type,
  count,
}: {
  type: "stars" | "forks";
  count: number;
}) {
  const renderCount = (n: number) =>
    n > 1000 ? `${(n / 1000).toFixed(1)}k` : n;

  return (
    <div className="border-color-brand surface-subtle flex min-w-[100px] flex-col items-end justify-end gap-4 rounded-md border px-4 py-4">
      <p className="text-fg font-mono text-sm tracking-wide uppercase opacity-50">
        {type === "stars" ? "Stars" : "Forks"}
      </p>
      <h3 className="text-fg text-right font-mono text-2xl font-medium">
        {renderCount(count)}
      </h3>
    </div>
  );
}

function Avatar({ username, avatar }: { username: string; avatar: string }) {
  return (
    <a
      href={`https://github.com/${username}`}
      target="_blank"
      rel="noopener noreferrer"
      className="size-10 shrink-0 cursor-pointer overflow-hidden rounded-xs border-2 border-neutral-200 bg-neutral-100 transition-all hover:scale-110 hover:border-neutral-400"
    >
      <img
        src={avatar}
        alt={`${username}'s avatar`}
        className="h-full w-full object-cover"
      />
    </a>
  );
}

export function GitHubOpenSource() {
  const githubStats = useGitHubStats();

  const STARS_COUNT = githubStats.data?.stars ?? GITHUB_LAST_SEEN_STARS;
  const FORKS_COUNT = githubStats.data?.forks ?? GITHUB_LAST_SEEN_FORKS;

  return (
    <section id="opensource">
      <div className="mx-auto max-w-5xl px-4 py-16">
        <h2 className="text-fg border-color-brand mb-8 border-b pb-8 font-mono text-2xl tracking-wide md:text-4xl">
          Open source
        </h2>
        <div
          className={cn([
            "flex flex-col items-center gap-6",
            "md:flex-row md:items-center md:justify-between md:gap-12",
          ])}
        >
          <div className="flex flex-col items-center gap-4 md:items-start">
            <p className="text-fg-muted max-w-md text-center text-base leading-relaxed md:text-left">
              Char values privacy and community, so it's been transparent from
              day one.
            </p>
            <a
              href={`https://github.com/${GITHUB_ORG_REPO}`}
              target="_blank"
              rel="noopener noreferrer"
              className={cn([
                "group inline-flex h-14 items-center justify-center gap-2 px-8",
                "text-fg border-color-brand rounded-full border",
                "hover:scale-[102%] hover:bg-[var(--color-brand-dark)] hover:text-white active:scale-[98%]",
                "cursor-pointer transition-all",
              ])}
            >
              <Icon icon="mdi:github" className="text-lg" />
              View on GitHub
            </a>
          </div>
          <div className="flex shrink-0 gap-2">
            <StatBadge type="stars" count={STARS_COUNT} />
            <StatBadge type="forks" count={FORKS_COUNT} />
          </div>
        </div>

        <div className="mt-12 flex flex-wrap justify-start gap-1">
          {CURATED_PROFILES.map((profile) => (
            <Avatar
              key={profile.username}
              username={profile.username}
              avatar={profile.avatar}
            />
          ))}
        </div>
      </div>
    </section>
  );
}
