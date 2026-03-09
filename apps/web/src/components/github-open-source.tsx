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
    <div className="flex h-[84px] w-[84px] flex-col items-center justify-center gap-1 rounded-xs border border-neutral-200 bg-neutral-100 px-4 text-stone-600">
      <p className="font-serif text-sm font-semibold">
        {type === "stars" ? "Stars" : "Forks"}
      </p>
      <p className="text-center text-sm font-medium">{renderCount(count)}</p>
    </div>
  );
}

function OpenSourceButton({
  showStars = false,
  starCount,
}: {
  showStars?: boolean;
  starCount?: number;
}) {
  const renderCount = (n: number) =>
    n > 1000 ? `${(n / 1000).toFixed(1)}k` : n;

  return (
    <div className="mx-auto flex w-full max-w-md flex-col items-center gap-4 text-center">
      <h2 className="font-serif text-2xl text-stone-700">Open source</h2>
      <p className="text-neutral-600">
        {
          "Char values privacy and community, so it's been transparent from day one."
        }
      </p>
      <a
        href={`https://github.com/${GITHUB_ORG_REPO}`}
        target="_blank"
        rel="noopener noreferrer"
        className={cn([
          "group inline-flex h-12 items-center justify-center gap-2 px-6",
          "rounded-full bg-linear-to-t from-neutral-800 to-neutral-700 text-white",
          "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
          "cursor-pointer transition-all",
        ])}
      >
        <Icon icon="mdi:github" className="text-lg" />
        View on GitHub
        {showStars && starCount && (
          <>
            <span className="text-neutral-400">•</span>
            <div className="flex items-center gap-1">
              <Icon icon="mdi:star" className="text-lg" />
              <span>{renderCount(starCount)}</span>
            </div>
          </>
        )}
      </a>
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

function GridRow({ children }: { children: React.ReactNode }) {
  return <div className="flex items-center gap-1">{children}</div>;
}

export function GitHubOpenSource() {
  const githubStats = useGitHubStats();

  const STARS_COUNT = githubStats.data?.stars ?? GITHUB_LAST_SEEN_STARS;
  const FORKS_COUNT = githubStats.data?.forks ?? GITHUB_LAST_SEEN_FORKS;

  return (
    <section>
      <div className="px-4 py-8">
        <div className="mx-auto max-w-4xl lg:hidden">
          <OpenSourceButton showStars={true} starCount={STARS_COUNT} />
        </div>

        <div className="mx-auto hidden max-w-7xl items-center justify-between lg:flex">
          <div className="flex gap-1">
            <div className="flex flex-col gap-1">
              <GridRow>
                {CURATED_PROFILES.slice(0, 2).map((profile) => (
                  <Avatar
                    key={`left-c1-r1-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(2, 4).map((profile) => (
                  <Avatar
                    key={`left-c1-r2-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(4, 6).map((profile) => (
                  <Avatar
                    key={`left-c1-r3-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(6, 8).map((profile) => (
                  <Avatar
                    key={`left-c1-r4-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
            </div>

            <div className="flex flex-col gap-1">
              <GridRow>
                {CURATED_PROFILES.slice(8, 10).map((profile) => (
                  <Avatar
                    key={`left-c2-r1-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <StatBadge type="stars" count={STARS_COUNT} />
              <GridRow>
                {CURATED_PROFILES.slice(10, 12).map((profile) => (
                  <Avatar
                    key={`left-c2-r4-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
            </div>

            <div className="flex flex-col gap-1">
              <GridRow>
                {CURATED_PROFILES.slice(12, 15).map((profile) => (
                  <Avatar
                    key={`left-c3-r1-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(15, 18).map((profile) => (
                  <Avatar
                    key={`left-c3-r2-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(18, 21).map((profile) => (
                  <Avatar
                    key={`left-c3-r3-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(21, 24).map((profile) => (
                  <Avatar
                    key={`left-c3-r4-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
            </div>
          </div>

          <div className="flex items-center justify-center">
            <OpenSourceButton />
          </div>

          <div className="flex gap-1">
            <div className="flex flex-col gap-1">
              <GridRow>
                {CURATED_PROFILES.slice(24, 27).map((profile) => (
                  <Avatar
                    key={`right-c1-r1-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(27, 30).map((profile) => (
                  <Avatar
                    key={`right-c1-r2-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(30, 33).map((profile) => (
                  <Avatar
                    key={`right-c1-r3-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(33, 36).map((profile) => (
                  <Avatar
                    key={`right-c1-r4-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
            </div>

            <div className="flex flex-col gap-1">
              <GridRow>
                {CURATED_PROFILES.slice(36, 38).map((profile) => (
                  <Avatar
                    key={`right-c2-r1-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <StatBadge type="forks" count={FORKS_COUNT} />
              <GridRow>
                {CURATED_PROFILES.slice(38, 40).map((profile) => (
                  <Avatar
                    key={`right-c2-r4-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
            </div>

            <div className="flex flex-col gap-1">
              <GridRow>
                {CURATED_PROFILES.slice(40, 42).map((profile) => (
                  <Avatar
                    key={`right-c3-r1-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(42, 44).map((profile) => (
                  <Avatar
                    key={`right-c3-r2-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(44, 46).map((profile) => (
                  <Avatar
                    key={`right-c3-r3-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
              <GridRow>
                {CURATED_PROFILES.slice(46, 48).map((profile) => (
                  <Avatar
                    key={`right-c3-r4-${profile.username}`}
                    username={profile.username}
                    avatar={profile.avatar}
                  />
                ))}
              </GridRow>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
