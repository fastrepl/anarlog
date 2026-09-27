# Mobile store distribution

Use only for explicitly requested mobile distribution. Inherit scope and
authorization from the [release coordinator](../SKILL.md); a desktop release
does not authorize mobile submission.

Read `apps/mobile/AGENTS.md`, `apps/mobile/app.json`,
`apps/mobile/app.config.ts`, `apps/mobile/eas.json`, the EAS build hook, and the
current `mobile_ci.yaml`. Use Expo's store-distribution skill when available and
verify commands against the installed EAS CLI and current official docs:

- https://docs.expo.dev/submit/ios/
- https://docs.expo.dev/submit/android/
- https://docs.expo.dev/submit/eas-json/
- https://docs.expo.dev/build-reference/app-versions/

### Identity, versions, and credentials

Run EAS commands from `apps/mobile` with `APP_VARIANT=stable`. Use the repository's
`stable` profile, not an assumed `production` profile. Pin and record the EAS CLI
version used for the release. Verify the signed-in account and project before
building or submitting. Current repository identities are:

- Project: `@john_fastrepl/anarlog-mobile`, ID `fcaa4e46-0da5-4dfc-a9e2-6447de0030d2`
- iOS bundle ID and Android package: `so.anarlog.mobile`
- App Store Connect app ID: `6807350358`
- EAS build environment: `production`; app variant: `stable`

Re-read these values rather than treating this list as authority if configuration
changes. Verify the selected Anarlog project and store destination before writes;
resource names alone do not establish authorization.

The mobile marketing version comes from `apps/mobile/release-version.json`
through `apps/mobile/app.config.ts`. Run
`node scripts/release-version.mjs --mobile --check` before building. Use
`node scripts/release-version.mjs --mobile <major.minor.patch>` to prepare a
new mobile version. Do not bump desktop `release-version.json` for a mobile
release, and do not edit the generated watch configuration from a mobile bump.
Keep `appVersionSource: remote` and `autoIncrement: true` for iOS build numbers
and Android version codes, and never reset those counters to match the marketing
version. Check remote build history and store versions before selecting a
build. Merge intentional version/profile changes before freezing the candidate.

Confirm signing and submission credential availability without printing secrets.
Use credentials already managed by EAS where possible. Google Play submission
requires the correct app record and a service account with access to its testing
track. Do not commit credential files or broaden account permissions to bypass a
failed submission.

### Native verification and candidate builds

Dispatch `mobile_ci.yaml` from `main` and verify its run SHA matches the mobile
candidate. Wait for `mobile_checks`, `ios_build`, `android_build`, `watchos_build`,
and the aggregate job; pull-request runs skip native builds.

- iOS dispatch rebuilds the CloudSync framework, runs `test-ios.sh` on a
  simulator, builds Release, and verifies embedded CloudSync and App Shortcuts.
- Android dispatch rebuilds all shipped CloudSync ABIs, builds Release, and
  checks the packaged libraries for the request-deadline patch marker. The
  current workflow does not execute the native CloudSync cancellation suite on
  Android. Report this coverage gap; a marker check is not a passing runtime test.
- Record the `cloudsync-ios-<sha>` and `cloudsync-android-<sha>` artifacts and all
  job results. Never report desktop tests as native mobile coverage.

Build from a clean checkout of the merged candidate, including Git LFS objects
and submodules. Do not upload a combined GitButler workspace or use `EAS_NO_VCS`
to hide source identity. If using an exported source archive, record its origin
SHA and hash and verify that no local changes entered it. The EAS post-install
hook must generate the native bridge before packaging. EAS builds use the
candidate's committed CloudSync bundle; a separate CI rebuild alone does not
prove which bytes are embedded in a signed store artifact.

For an exported source tree, resolve both `EAS_PROJECT_ROOT` and `apps/mobile`
to their physical paths before invoking EAS. Confirm their relative path is
exactly `apps/mobile`. On macOS, mixing `/tmp` with its physical `/private/tmp`
path produces an invalid project directory in the remote build job. Check the
job's `projectRootDirectory` before accepting a build.

```bash
APP_VARIANT=stable eas build --platform ios --profile stable --non-interactive --no-wait
APP_VARIANT=stable eas build --platform android --profile stable --non-interactive --no-wait
APP_VARIANT=stable eas build:view <build-id> --json
```

Record each build ID, source SHA, profile, app version, build number/version
code, status, artifact URL, and SHA-256. Verify an iOS device archive and Android
AAB, not a simulator build or development APK. Inspect packaged identity,
CloudSync inclusion, and signing metadata before submission. Never select
`--latest` when concurrent builds can select a different candidate.

### TestFlight and Google Play internal testing

Use the explicit completed build IDs:

```bash
APP_VARIANT=stable eas submit --platform ios --profile stable --id <ios-build-id> --non-interactive --no-auto-testflight-setup --wait
APP_VARIANT=stable eas submit --platform android --profile stable --id <android-build-id> --non-interactive --wait
```

For Google Play internal testing, require the selected submission profile to
specify `android.track: internal`. A `completed` release makes the build
available on that track; `draft` uploads it without completing rollout. Do not
silently switch to `production`. An EAS `distribution: internal` build is a
separate sideloading mechanism and is not Play internal testing.

Follow the returned submission URLs to terminal status. For iOS, verify Apple
processing completes and the exact version/build appears in TestFlight; check
availability to the intended existing tester group. External TestFlight testing
can require Beta App Review. For Android, verify the exact version code on the
internal track and the release status. Report missing credentials, app setup,
store processing, or tester-group access as concrete pending steps.

Current EAS Submit enables automatic TestFlight setup by default, which can
create a group and invite every App Store Connect admin. Keep
`--no-auto-testflight-setup` unless those invitations were explicitly requested.
An upload request alone does not authorize inviting additional testers.

### Public App Store and Google Play releases

Run only when public distribution was requested. EAS iOS submission uploads to
App Store Connect/TestFlight; public distribution additionally requires selecting
the build for an App Store version, completing metadata and review requirements,
submitting for review, and verifying the approved release becomes available.
Google Play public distribution requires an explicitly selected production
profile or promotion of the verified internal build, with the requested rollout
fraction and release status. Keep store review and public availability distinct
from a successful upload. Never accept new legal agreements on the user's behalf.
