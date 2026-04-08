# Instruction

- Read through the commits, and most of the diffs, but only keep the desktop-related thing to the changelog.
- All changelogs should "worth reading" for app users. No internal changes or infra updates.

# Scripts

1. This will give you all changelogs that we have now.

```bash
find packages/changelog/content -type f | while read f; do
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
gh api repos/fastrepl/char/compare/<>...<>  --jq '.commits'
```

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
