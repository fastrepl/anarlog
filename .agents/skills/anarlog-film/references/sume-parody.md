# AI skits and parody series (sume.com)

Short AI-generated skits that dramatize the problem (a meeting bot that never leaves, a cloud that keeps everything) and hand off to Anarlog. They run alongside the motion-graphics episodes, not instead of them. sume.com generates clips with several video models (Seedance, Kling, Veo, Wan) plus lip-sync.

## Credits and billing (hard rules)

- **Spend only from the existing balance, within a per-episode cap John sets** (for example "at most 400 credits for this skit"). Write the cap and the running total in the episode's `script.md` and stop at the cap.
- **Never trigger a top-up, plan change, subscription, auto-recharge or any billing screen action** without John's explicit OK for that exact action. If a generation would exceed the balance or the cap, stop and report.
- Price every generation before running it (model, duration, resolution). Prefer the cheapest model that reads at phone size; draft at the lowest resolution and length, then render the keeper once at full quality.
- Never sign in, create accounts or enter payment details. If sume.com needs any of that, hand it to John.

## Characters and tropes

- Use **fictional** companies, products and people only: `CloudCorp`, `CloudCorp Notetaker`, `Bot #47`, `The Retention Department`, `Sub-Processor Steve`. Recurring characters make a series; keep a character sheet (look, voice, catchphrase) in `~/Documents/anarlog-films/series/<name>/characters.md` and reuse the same reference images.
- **Never depict or voice a real person** (no executives, founders, celebrities, politicians), never use a real company's name, logo, product UI or trade dress, and never imply a real company did a specific thing. Satire targets the pattern (bots in calls, opaque retention, US-cloud exposure), not a named vendor.
- Keep claims about Anarlog inside the skit as true as in the films: no bot, local SQLite, choose your models, MIT. The joke is on CloudCorp, never on facts.
- No sensitive or harmful content: no violence, no real meetings or real audio, no minors, nothing a platform would flag as deceptive.

## Disclosure

- Switch on every platform's AI disclosure for these posts (YouTube "Altered or synthetic content", Meta "AI info", TikTok "AI-generated content").
- Put a label in the caption's first line where the format is parody: `AI parody. CloudCorp is fictional.`
- Burn a small `AI-GENERATED PARODY` mono tag into the first 3 seconds of the clip (top-left, inside the safe area).

## Production

1. Write the skit like a script: 3-5 shots, 15-30s total, one joke, one turn. Dialogue in straight quotes, no em dashes.
2. Generate shot by shot at draft quality; keep seeds and prompts in `script.md` so a keeper can be regenerated.
3. Lip-sync only the shots that need it.
4. Assemble with ffmpeg (concat, loudness normalise the skit's own audio).
5. **Splice the sovereign outro on every skit:** a 5s `anarlog-film` end card rendered from the template (newsprint, wordmark centred, line `No bots. Local SQLite. anarlog.so`), with a hard cut from the skit, no transition. Render it with `render.mjs` at the skit's aspect ratio (`FORMAT = "9:16"` for vertical) and concat it.
6. Review the full clip at phone size before showing John. Post only through `anarlog-film-publish`, with his approval per post.
