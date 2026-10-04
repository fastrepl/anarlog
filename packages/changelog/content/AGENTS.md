# Instruction

- Read through the commits and relevant diffs, keeping only user-facing changes for the requested release stream.
- Maintain Desktop notes in `desktop/<version>.md` and Mobile notes in `mobile/<version>.md`. Read the stream directory’s `AGENTS.md`.
- Root `<version>.md` files are generated compatibility copies for shipped desktop clients. Do not edit them directly. After editing desktop notes, run `node scripts/sync-desktop-changelogs.mjs` and include its output in the same commit; CI checks for drift.
- All changelogs should "worth reading" for app users. No internal changes or infra updates.
- If a user-facing change came from a pull request by someone outside the Fastrepl org, acknowledge them on that item. See [Contributor credit](#contributor-credit).
- Each changelog must include `date` and `summary` frontmatter. `summary` is shown on the web changelog index, so keep it to one concise, plain-text, user-facing sentence with no markdown or custom tags.

```md
---
date: "YYYY-MM-DD"
summary: "One concise, user-facing sentence for the changelog index preview."
---
```

# Scripts

1. This will give you all changelogs that we have now.

```bash
find packages/changelog/content/desktop -type f | while read f; do
  echo "============================================================"
  echo "FILE: $f"
  echo "------------------------------------------------------------"
  cat "$f"
  echo
done
```

2. This will give you what versions we actually have.

```bash
gh api repos/:owner/:repo/git/refs/tags --jq '.[] | select(.ref | startswith("refs/tags/desktop_v1")).ref' | sed 's#refs/tags/##' |
while read tag; do
  if gh api repos/:owner/:repo/git/tags/$tag --jq '.tagger.date' >/tmp/tagdate 2>/dev/null; then
    date=$(cat /tmp/tagdate)
  else
    sha=$(git rev-parse $tag)
    date=$(gh api repos/:owner/:repo/commits/$sha --jq '.commit.author.date')
  fi
  echo "$tag  $date"
done
```

3. To actually see what's changed between two versions, you can use this.

```bash
gh api repos/fastrepl/anarlog/compare/<>...<>  --jq '.commits'
```

4. For each merged PR in that range, look up who opened it so outside
   contributors can be credited:

```bash
gh api repos/fastrepl/anarlog/pulls/<number> --jq '{login: .user.login, type: .user.type, association: .author_association}'
```

# Contributor credit

If a user-facing change came from a pull request by someone else — not a
Fastrepl org member, collaborator, owner, or bot — acknowledge them on that
changelog item. Team PRs need no credit line.

Credit when `author_association` is not `MEMBER`, `OWNER`, or `COLLABORATOR`,
and `user.type` is not `Bot`. Use the pull request author, not the merger.

Put the thanks at the end of the item, after the user-facing sentence. Link
the GitHub username. Do not put credits in `summary`. Skip credit when the
change is internal-only and does not appear in the changelog.

```md
- Use the actual default microphone on Linux instead of silently recording
  from ALSA's null device. Thanks [@jacopone](https://github.com/jacopone).
```

If several people outside the org authored the same item, thank each of them.

# Custom Tags

The desktop changelog renderer (Streamdown-based) supports custom HTML tags beyond standard markdown.

## `<banner>`

Use for announcements, important notices, or highlights.

Attributes:

- `title` (optional): Bold heading text at the top of the banner.
- `variant` (optional): `"warning"` for amber/yellow style, `"info"` for blue style. Defaults to amber/info style.

```mdx
<banner title="Hyprnote is now Char!">
We've renamed the app. All your data is safe and nothing changes on your end.
</banner>

<banner title="Breaking Change" variant="warning">
The old plugin format is no longer supported. Please update your plugins.
</banner>
```

## Release streams and channels

Each release is identified by stream and version. Desktop and Mobile version
independently; never compare their semantic versions to sort a combined feed.
Website feeds sort by actual publication time. Keep OS-specific changes within
the parent stream while those releases remain coupled. Add a new stream only
when another surface has its own release lifecycle and user-facing notes.

Preparing or merging notes does not publish them. Desktop entries require a
published, non-draft, non-prerelease GitHub `desktop_v<version>` release. Mobile
entries require a verified publication record; see `mobile/AGENTS.md`. Production
builds omit unpublished notes from the index, direct URLs, and browser bundles.
Local development previews drafts. A planned frontmatter date is never a gate.

Nightly publication is retired. Preserve historical notes without generating
new Nightly entries or publishing them on the website. Stable notes cover the
full delta since the preceding stable release, including changes previously
seen in preview builds.
