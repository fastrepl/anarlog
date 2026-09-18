#!/usr/bin/env bash
# Merge upstream into this fork with the two mechanical conflict classes handled.
#
# This fork diverges from upstream in three ways: it deletes whole apps, it
# renames the product, and it changes behaviour. Only the third needs a human.
# The first two are resolved the same way every time, so they are scripted:
# deletions are re-applied, and the brand is re-derived from branding/brand.json.
# Whatever conflict survives is a genuine overlap worth reading.
#
# Usage: scripts/merge-upstream.sh [ref]     (default: upstream/main)

set -euo pipefail

cd "$(dirname "$0")/.."
REF="${1:-upstream/main}"
REMOVED_LIST=branding/removed-paths.txt

if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "working tree has uncommitted changes -- commit or stash first" >&2
  exit 1
fi

REMOTE="${REF%%/*}"
if [ "$REMOTE" != "$REF" ] && git remote | grep -qx "$REMOTE"; then
  echo "==> fetching $REMOTE"
  git fetch "$REMOTE"
fi

echo "==> merging $REF"
git merge --no-edit "$REF" && MERGE_CLEAN=1 || MERGE_CLEAN=0

# Re-apply the deletions. `git rm` is the correct resolution for a modify/delete
# conflict on a path this fork dropped, and it also catches files upstream has
# newly added under a removed prefix (those arrive merged, not conflicted).
# macOS still ships bash 3.2, so no mapfile and no `readarray` here.
PREFIXES=()
while IFS= read -r line; do
  [ -n "$line" ] && PREFIXES[${#PREFIXES[@]}]="$line"
done < <(grep -vE '^[[:space:]]*(#|$)' "$REMOVED_LIST")

DROPPED=0
if [ ${#PREFIXES[@]} -gt 0 ]; then
  while IFS= read -r path; do
    for prefix in "${PREFIXES[@]}"; do
      case "$path" in
        "$prefix"*)
          git rm -rq --ignore-unmatch -- "$path" 2>/dev/null || true
          DROPPED=$((DROPPED + 1))
          break
          ;;
      esac
    done
  done < <(git ls-files | sort -u)
fi
[ "$DROPPED" -gt 0 ] && echo "==> re-applied deletion for $DROPPED path(s) from $REMOVED_LIST"

echo "==> re-deriving the brand"
node branding/apply.mjs
git add -A -- . ':!*.orig'

REMAINING=$(git diff --name-only --diff-filter=U)
if [ -n "$REMAINING" ]; then
  echo
  echo "==> conflicts left for you (real divergence, not brand or deletions):"
  printf '%s\n' "$REMAINING" | sed 's/^/    /'
  echo
  echo "resolve, then: git add <files> && git commit"
  exit 1
fi

if [ "$MERGE_CLEAN" = 0 ]; then
  echo "==> all conflicts were mechanical; committing the merge"
  git commit --no-edit
else
  git diff --cached --quiet || git commit -m "chore: re-derive brand after merging $REF"
fi

echo "==> done"
