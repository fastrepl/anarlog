# Mobile release notes

Cover mobile user-facing changes for the requested version and audience. Use
iOS, Android, and watchOS sections for platform-specific behavior. Desktop
release notes and desktop version numbers do not establish mobile coverage.
Use the marketing version in `apps/mobile/release-version.json`; keep build
numbers/version codes separate. Stable notes include all changes since the
preceding stable mobile release, including those already tested in beta.

Prepare `<version>.md` with `date` and plain-text `summary` before candidate
freeze. Follow the parent writing and contributor-credit instructions. Do not
invent historical notes or publication evidence.

## Publication record

Only after the exact build is available to its intended audience, create or
update the adjacent `<version>.json` record. An absent record keeps the notes
out of deployable website builds, including direct URLs and browser bundles.
A build, upload, or submission success is not availability. Verify TestFlight
processing and intended tester-group access, or the selected Play track's
rollout. Public App Store availability requires approval and release.

The record contains `version`, the full 40-character candidate `sourceSha`,
`channel` (`beta` or `stable`), and a nonempty `availability` array. Each item
contains:

- `platform`: `ios` or `android`, at most one entry per platform.
- `destination`: `testflight` for iOS beta; `play-internal` or `play-beta` for
  Android beta; `app-store` or `play-production` for stable.
- `build`: the exact iOS build number or Android version code, as a string.
- `publishedAt`: the verified availability time with timezone, in ISO 8601.
- `evidenceUrl`: an HTTPS link to the exact store/build/rollout evidence.

Record only available platforms. Add the second platform when it becomes
available, preserving the first platform's original publication time. The
website labels the channel and actual destinations; it never infers Android
availability from TestFlight or vice versa. Build numbers, source SHA, and
private evidence links remain build-time data and are excluded from public
website bundles.

Merge the verified record and publish the website within the authorized mobile
release operation. Verify `/changelog/mobile/<version>/`, the Mobile filter,
and the combined feed. Future public promotion of a beta version must update
the channel and verified destinations together; never mix beta-only availability
into a stable record.
