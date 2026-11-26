---
name: anarlog-film-publish
description: "Use when posting or scheduling an Anarlog film or AI skit (anarlog-film episodes, launch clips, sume parodies) to Anarlog's own accounts on X, LinkedIn, Reddit, and YouTube, Reels, Shorts or TikTok once those exist: cutting the 9:16 version, captions, drafting and auditing copy in Anarlog's company voice, AI disclosure, getting John's approval per post, scheduling, and checking it landed."
---

# Publishing Anarlog films

One film becomes two cuts and a handful of posts, all on **Anarlog's own product accounts**. Make the film with the `anarlog-film` skill first; this skill covers everything after the MP4 exists.

| Cut                                 | File                     | Goes to                                   |
| ----------------------------------- | ------------------------ | ----------------------------------------- |
| 16:9, 1920x1080                     | `epNN-vK.mp4`            | X, LinkedIn, Reddit, YouTube (long-form)  |
| 9:16, 1080x1920, captions burned in | `epNN-vK-vertical.mp4`   | YouTube Shorts, Instagram Reels, TikTok   |
| Captions file                       | `epNN.srt`               | Uploaded with the 16:9 cut on X, YouTube  |

Canonical storage is `~/Documents/anarlog-films/<epNN-slug>/` (never `~/Downloads`). Keep a `posts.md` there with every approved post, its slot, and its live URL.

## Accounts

| Platform        | Account                                                         | State                |
| --------------- | --------------------------------------------------------------- | -------------------- |
| X               | `@anarlogapp`                                                   | active               |
| LinkedIn        | Anarlog page, `https://www.linkedin.com/company/anarlog/`       | active               |
| Reddit          | `r/anarlog`, posted by its moderator                            | active               |
| YouTube, Shorts | Anarlog channel                                                 | only once it exists  |
| Instagram Reels | Anarlog account                                                 | only once it exists  |
| TikTok          | Anarlog account                                                 | only once it exists  |

- **Never post Anarlog films to Char's accounts** (`@char_notepad`, `/company/charnotepad`, `r/charapp`) and never Char films to Anarlog's.
- **Never use John's personal profiles for the primary post.** After the company post is live, John may repost or quote it from his personal accounts with an optional one-line note; first person belongs only there.
- **Never create accounts, sign in, switch accounts, or enter passwords or 2FA codes.** If a platform needs any of that, stop and hand it to John. Switching from John's identity to the Anarlog page is an account action: get his OK or have him switch.

## Hard rules

- **Nothing goes out without John's OK for that specific post**: one platform, one account, the exact copy, the exact file, the exact time. A changed word or slot needs a new OK. "Schedule the series" is not approval for any post.
- **Company voice.** Anarlog speaks as Anarlog: second person for what the reader does, plain statements for what Anarlog does. No "I", "my", "we think". Specific claims over slogans.
- **Straight quotes only, no em dashes or en dashes**, in every post, title, description, caption and comment.
- **Claims match the film and the code.** A post may only say what the film shows; the episode's "Must not say" list in `script.md` applies to posts. No certification claims, no self-hosted sync claims until shipped, no named competitors.
- **AI disclosure.** The narration is John's voice clone and skits are AI video, so switch on each platform's AI label where one exists: YouTube "Altered or synthetic content: Yes", Instagram "AI info", TikTok "AI-generated content". X, LinkedIn and Reddit have no switch; add nothing to the text for narrated episodes.
- **Parody.** Any AI skit (see `anarlog-film/references/sume-parody.md`) says so in the caption's first line on every platform: `AI parody. CloudCorp is fictional.`
- **Audit every piece of public copy before John sees it** with the writing audit in `ComputelessComputer/writing` (`gh api repos/ComputelessComputer/writing/contents/audit/SKILL.md --jq .content | base64 -d`, plus its `references/style-profile.md`). Presets: `twitter` for X, Reels, TikTok and Shorts; `linkedin`; `reddit`; `blog` for YouTube descriptions.

## 1. Build the vertical version

```bash
S=.agents/skills/anarlog-film/scripts
uv run --with faster-whisper python3 $S/captions.py film-voice.mp4 film.srt narration.txt
python3 $S/vertical-page.py film.html film-vertical.html --srt film.srt
node $S/render.mjs film-vertical.html frames frames-vertical
MUSIC=music.wav bash $S/mux.sh frames-vertical film-vertical.mp4 narration-1.mp3 <NARRATION_AT>
```

Read the SRT before using it (product terms spelled from `narration.txt`). Render a vertical contact sheet and check the safe area: Reels, Shorts and TikTok cover roughly the top 200px, the bottom 400px and the right 120px. Headline, demo and captions stay clear; only the docked mark and the rail sit in the bottom strip.

## 2. Platform specs

Limits change; check the upload screen when a number matters.

|               | X (`@anarlogapp`)                         | LinkedIn (Anarlog page)                              | Reddit (`r/anarlog`)                   |
| ------------- | ----------------------------------------- | ---------------------------------------------------- | -------------------------------------- |
| Cut           | 16:9                                      | 16:9                                                 | 16:9, uploaded in the post             |
| Text          | 280 per post                              | 3,000; the hook inside ~210 (See more fold)          | title 300; body as needed              |
| Hashtags      | 0-1                                       | 0-3                                                  | none                                   |
| Captions      | upload the SRT                            | auto captions off; the 16:9 cut has none burned in   | none                                   |
| Links         | not in the body; `anarlog.so` in a reply  | not in the post; `anarlog.so` in the first comment   | in the body                            |
| AI disclosure | none                                      | none                                                 | none                                   |
| Schedule with | X scheduler: post + reply as one thread   | composer, Schedule post; first comment by hand       | moderator Schedule button beside Post  |

|               | YouTube                            | YouTube Shorts                       | Instagram Reels                 | TikTok                               |
| ------------- | ---------------------------------- | ------------------------------------ | ------------------------------- | ------------------------------------ |
| Cut           | 16:9                               | 9:16                                 | 9:16                            | 9:16                                 |
| Text          | title 100, description 5,000       | title 100                            | caption 2,200 (about 125 show)  | caption 4,000                        |
| Captions      | upload the SRT                     | burned in                            | burned in; auto captions off    | burned in; auto captions off         |
| Links         | in the description                 | Related video: the long-form episode | bio only                        | bio only                             |
| AI disclosure | Altered or synthetic content: Yes  | same                                 | AI info                         | AI-generated content                 |

Shared for vertical posts: 1080x1920, H.264, AAC 48 kHz, faststart MP4 (`mux.sh` writes this). Pick a cover frame where the headline shows.

## 3. Draft, audit, approve

1. **Read what the film says** from `script.md` and the SRT, not memory.
2. **Draft one post per platform**, starting from X. Lead with the film's one specific claim (where the audio goes, who can read it), end with the series marker (working title `Anarlog Field Files, ep 01`; confirm the series name with John before the first post).
   - X: one post plus a reply with only `https://anarlog.so`.
   - LinkedIn: 3-5 short paragraphs, hook in the first line, link in the first comment (as Anarlog).
   - Reddit: the YouTube-style title; body is the X text, then the link.
   - YouTube: title plus description with the link; Shorts: title plus Related video.
   - John's optional repost note: one line, first person is fine there.
3. **Run the audit** on each draft with its preset; keep the revised text and what changed.
4. **Send John one approval message**: per post, the platform, account, file, exact text, slot (KST and ET), AI-disclosure setting, and a one-line audit summary. He may approve some and edit others.
5. **Approval covers that post only.** Any change to copy, file or slot needs a new OK.

## 4. Cadence

Post in one slot across every Anarlog account, then John reposts. Default slot: **11:00 PM KST** while the US is on standard time (9 AM ET); **10:00 PM KST** during US daylight time. Always state both times in the approval message.

## 5. Schedule

Use the browser John is already signed into (for example Aside: run `aside guide` first, one platform per run, and watch it).

- Copy each MP4 (and the SRT) into the browser tool's upload folder first.
- Pass the approved text verbatim, the account, the file and the slot. Tell the agent to stop and report on any sign-in, verification, new-account flow, or unexpected account.
- **X:** compose from the loaded `@anarlogapp` home timeline (X can post from the wrong account even when the switcher shows the right one). Add the video post first, then Add post for the link reply, then Schedule: X schedules the pair as one thread. Added the other way round, X drops the schedule.
- **LinkedIn:** post as the Anarlog page from the page admin view; the first comment cannot be scheduled, so post it by hand as Anarlog after the slot.
- **Reddit:** the moderator Schedule button needs a title first; the post goes out as the moderator who scheduled it.
- **YouTube, Reels, TikTok:** only once John has created the account and signed in himself. Confirm the handle on the profile before uploading.

## 6. Check after scheduling and after it goes live

**Right after scheduling:** the post is in the platform's scheduled queue with the approved time, the exact text, the right account, the video attached with the right duration, the SRT and AI disclosure where they apply, and no duplicate or early-published copy. Report each as "scheduled, verified in queue", never from a success toast.

**After the slot:** open each post from the account's own profile URL; confirm author, text and playback; post the link comments that could not be scheduled; check burned-in captions on a phone for vertical cuts. Then John reposts from his personal accounts if he wants to. Log every live URL in `posts.md`.

## Related

- `anarlog-film`: making the film, the vertical page, and AI skits.
- Root `AGENTS.md`: product terms and component ownership.
