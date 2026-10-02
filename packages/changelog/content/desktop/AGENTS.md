# Desktop release notes

Cover desktop user-facing changes since the preceding stable release. Use
macOS, Windows, and Linux sections when a change applies only to an OS; keep
one release stream while they share versions.

Files are `<major>.<minor>.<patch>.md`. Preserve `date`, plain-text `summary`,
and external-contributor credits from the parent instructions. Website
publication requires the corresponding published stable GitHub release.

Run `node scripts/sync-desktop-changelogs.mjs` after each edit. Root copies
keep the Markdown URLs used by shipped desktop builds working; they are
generated output, not another maintained source.
