import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { Check } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import { cn } from "@anlg/utils";

import { collectBadges, useCollectedBadges } from "./badge-queries";
import {
  type BadgeId,
  getBadgeProgress,
  type parseCollectedBadges,
} from "./badges";
import type { ActivityRecord } from "./queries";
import { ProgressBar } from "./tremor/progress-bar";

import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { DEFAULT_USER_ID } from "~/shared/utils";
import { commands } from "~/types/tauri.gen";

export function BadgeCollection(props: {
  records: ActivityRecord[];
  now: Date;
  timezone?: string;
  weekStartsOn: 0 | 1;
}) {
  const ownerId = DEFAULT_USER_ID;
  return <PersonalBadges key={ownerId} {...props} ownerId={ownerId} signedUp />;
}

function PersonalBadges({
  ownerId,
  ...inputs
}: {
  ownerId: string;
  records: ActivityRecord[];
  signedUp: boolean;
  now: Date;
  timezone?: string;
  weekStartsOn: 0 | 1;
}) {
  const collection = useCollectedBadges(ownerId);
  const onboarding = useQuery({
    queryKey: ["badges", "onboarding-complete"],
    queryFn: async () => {
      const result = await commands.getOnboardingNeeded();
      if (result.status === "error") throw new Error(result.error);
      return !result.data;
    },
  });
  if (collection.error || onboarding.error) {
    return (
      <p role="alert" className="text-muted-foreground text-sm">
        <Trans>Couldn't load your badges. Reopen this page to try again.</Trans>
      </p>
    );
  }
  if (collection.isLoading || onboarding.isPending) {
    return (
      <p role="status" className="text-muted-foreground text-sm">
        <Trans>Loading your badges…</Trans>
      </p>
    );
  }
  const collected = collection.data ?? {};
  const progress = getBadgeProgress({
    ...inputs,
    onboardingComplete: onboarding.data,
  });
  const newBadges = progress
    .filter((badge) => badge.value >= badge.target && !collected[badge.id])
    .map((badge) => badge.id);
  return (
    <>
      <BadgeGallery progress={progress} collected={collected} />
      {newBadges.length > 0 && (
        <CollectNewBadges
          key={newBadges.join(",")}
          ownerId={ownerId}
          ids={newBadges}
        />
      )}
    </>
  );
}

function CollectNewBadges({
  ownerId,
  ids,
}: {
  ownerId: string;
  ids: BadgeId[];
}) {
  const mutation = useMutation({
    mutationFn: () => collectBadges(ownerId, ids),
  });
  useMountEffect(() => {
    mutation.mutate();
  });
  if (!mutation.isError) return null;
  return (
    <div
      role="alert"
      className="text-muted-foreground flex items-center gap-3 text-xs"
    >
      <Trans>Couldn't save your new badges.</Trans>
      <Button size="sm" variant="ghost" onClick={() => mutation.mutate()}>
        <Trans>Try again</Trans>
      </Button>
    </div>
  );
}

export function BadgeGallery({
  progress,
  collected,
}: {
  progress: ReturnType<typeof getBadgeProgress>;
  collected: ReturnType<typeof parseCollectedBadges>;
}) {
  const { t, i18n } = useLingui();
  const [selectedId, setSelectedId] = useState<BadgeId | null>(null);
  const details = {
    hello: {
      name: t`Hello, BlackMushi`,
      description: t`Create your BlackMushi account. A place for your conversations to call home.`,
    },
    "all-set": {
      name: t`All Set`,
      description: t`Complete onboarding. You're ready for your next conversation.`,
    },
    "first-words": {
      name: t`First Words`,
      description: t`Capture your first conversation. Every collection starts somewhere.`,
    },
    "good-listener": {
      name: t`Good Listener`,
      description: t`Capture 10 conversations. More moments you can return to.`,
    },
    "memory-keeper": {
      name: t`Memory Keeper`,
      description: t`Capture 50 conversations. A growing collection of ideas and decisions.`,
    },
    "story-collector": {
      name: t`Story Collector`,
      description: t`Capture 100 conversations. A hundred stories, saved in your own words.`,
    },
    "living-library": {
      name: t`Living Library`,
      description: t`Capture 250 conversations. Your own library of shared knowledge.`,
    },
    "finding-rhythm": {
      name: t`Finding Your Rhythm`,
      description: t`Capture conversations in 4 different weeks. They don't need to be consecutive.`,
    },
    "familiar-face": {
      name: t`Familiar Face`,
      description: t`Capture conversations in 12 different weeks. A little at a time, at your own pace.`,
    },
  };
  const number = new Intl.NumberFormat(i18n.locale);
  const count = number.format(Object.keys(collected).length);
  const total = number.format(progress.length);
  const badges = progress.map((badge) => {
    const value = number.format(badge.value);
    const target = number.format(badge.target);
    return {
      ...badge,
      ...details[badge.id],
      collectedAt: collected[badge.id],
      progressLabel:
        badge.metric === "conversations"
          ? t`${value} / ${target} conversations`
          : badge.metric === "weeks"
            ? t`${value} / ${target} active weeks`
            : badge.metric === "signup"
              ? t`Create your account`
              : t`Complete onboarding`,
    };
  });
  const selected = badges.find((badge) => badge.id === selectedId);

  return (
    <section className="flex flex-col gap-5" aria-label={t`Your badges`}>
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <div className="flex flex-col gap-1">
          <h3 className="text-sm font-medium">
            <Trans>Your badges</Trans>
          </h3>
          <p className="text-muted-foreground text-xs">
            <Trans>
              Little steps worth keeping. Collect them at your own pace.
            </Trans>
          </p>
        </div>
        <span className="text-muted-foreground text-xs">
          <Trans>
            {count} of {total} collected
          </Trans>
        </span>
      </div>
      <ul className="grid grid-cols-2 gap-3 min-[480px]:grid-cols-3">
        {badges.map((badge) => (
          <li key={badge.id}>
            <button
              type="button"
              onClick={() => setSelectedId(badge.id)}
              aria-label={badge.name}
              className="bg-background border-border hover:bg-muted focus-visible:outline-ring isolate flex h-full w-full cursor-pointer flex-col items-center gap-3 rounded-2xl border px-3 py-5 text-center focus-visible:outline-2 focus-visible:outline-offset-2"
            >
              <BadgeEmblem id={badge.id} collected={!!badge.collectedAt} />
              <span className="flex flex-col gap-1">
                <span className="text-sm font-medium">{badge.name}</span>
                <span className="text-muted-foreground text-xs">
                  {badge.collectedAt ? (
                    <Trans>Collected</Trans>
                  ) : (
                    badge.progressLabel
                  )}
                </span>
              </span>
            </button>
          </li>
        ))}
      </ul>
      <p className="text-muted-foreground text-xs">
        <Trans>
          Collected badges stay yours on this device, even when you delete a
          note or take a break. Imported transcripts count; the welcome demo
          doesn't.
        </Trans>
      </p>
      <Dialog
        open={!!selected}
        onOpenChange={(open) => {
          if (!open) setSelectedId(null);
        }}
      >
        {selected && (
          <DialogContent className="isolate max-w-sm rounded-2xl">
            <div className="flex justify-center py-2">
              <BadgeEmblem
                id={selected.id}
                large
                collected={!!selected.collectedAt}
              />
            </div>
            <DialogTitle className="text-center">{selected.name}</DialogTitle>
            <DialogDescription className="text-center">
              {selected.description}
            </DialogDescription>
            {selected.collectedAt ? (
              <p className="text-muted-foreground text-center text-xs">
                <Trans>
                  Collected{" "}
                  {new Intl.DateTimeFormat(i18n.locale, {
                    dateStyle: "medium",
                  }).format(new Date(selected.collectedAt))}
                </Trans>
              </p>
            ) : (
              <div className="flex flex-col gap-3">
                <p className="text-muted-foreground text-center text-xs">
                  {selected.progressLabel}
                </p>
                <ProgressBar
                  aria-label={selected.name}
                  value={selected.value}
                  max={selected.target}
                />
              </div>
            )}
          </DialogContent>
        )}
      </Dialog>
    </section>
  );
}

function BadgeEmblem({
  id,
  collected,
  large = false,
}: {
  id: BadgeId;
  collected: boolean;
  large?: boolean;
}) {
  return (
    <span
      className={cn([
        "relative inline-flex shrink-0",
        large ? "size-36" : "size-24",
      ])}
      aria-hidden="true"
    >
      <img
        src={`/assets/badges/${id}.webp`}
        alt=""
        width={512}
        height={512}
        draggable={false}
        className={cn([
          "size-full object-contain mix-blend-multiply dark:mix-blend-screen dark:invert",
          !collected && "opacity-30",
        ])}
      />
      {collected && (
        <span className="bg-background border-border rounded-pill absolute right-0 bottom-0 flex size-5 items-center justify-center border">
          <Check className="size-3" />
        </span>
      )}
    </span>
  );
}
