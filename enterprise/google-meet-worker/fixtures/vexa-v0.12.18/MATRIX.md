# Vexa v0.12.18 Google Meet behavior matrix

Pinned reference: Vexa `v0.12.18`, commit `1b62993e7e97c6ee04a5dcb116f7749ec74169df`.
These fixtures are replayable snapshots of normalized Anarlog admission/runtime classifiers. They do not require Vexa internals at test time.

| Fixture                             | Vexa module                           | Anarlog outcome                   | Terminal reason             | Retryable |
| ----------------------------------- | ------------------------------------- | --------------------------------- | --------------------------- | --------- |
| `admission/host-denied.json`        | `join/src/googlemeet/admission.ts`    | Rejected HostDenied               | `admission_denied`          | no        |
| `admission/waiting-room.json`       | `join/src/googlemeet/admission.ts`    | WaitingForAdmission               | —                           | —         |
| `admission/admitted.json`           | `join/src/googlemeet/join.ts`         | Admitted                          | —                           | —         |
| `admission/captcha-unsolved.json`   | `join/src/googlemeet/admission.ts`    | Rejected CaptchaUnsolved          | `authentication_failed`     | no        |
| `admission/error-page.json`         | `join/src/googlemeet/admission.ts`    | Rejected ErrorPage                | `provider_error`            | no        |
| `admission/consent.json`            | `join/src/googlemeet/admission.ts`    | ConsentRequired                   | —                           | —         |
| `runtime/removed.json`              | `join/src/googlemeet/removal.ts`      | Removed                           | `removed_from_meeting`      | no        |
| `runtime/meeting-ended.json`        | `join/src/googlemeet/removal.ts`      | MeetingEnded                      | `meeting_ended`             | no        |
| `runtime/network-lost.json`         | `join/src/googlemeet/removal.ts`      | NetworkLost after grace           | `network_lost`              | yes       |
| `runtime/active.json`               | `gmeet-capture/src/gmeet-capture.ts`  | Active                            | —                           | —         |
| `runtime/silence.json`              | `gmeet-capture/src/pcm-capture.ts`    | Active (no tiles besides bot)     | `no_one_joined` after grace | yes       |
| `runtime/nobody-joined.json`        | `gmeet-capture/src/pcm-capture.ts`    | Active until empty-room grace     | `no_one_joined`             | yes       |
| `runtime/overlapping-speakers.json` | `gmeet-capture/src/gmeet-speakers.ts` | Active with two named tiles       | —                           | —         |
| `runtime/speaker-renamed.json`      | `gmeet-capture/src/gmeet-speakers.ts` | Active with renamed tile          | —                           | —         |
| `runtime/unresolved-speaker.json`   | `gmeet-capture/src/gmeet-speakers.ts` | Active with chrome chrome-ui tile | —                           | —         |
| `runtime/long-duration.json`        | `gmeet-capture/src/gmeet-capture.ts`  | Active after two hours            | —                           | —         |

Scenarios in `scenarios.json` replay the same snapshots through `WorkerLifecycle` and assert the provider-neutral terminal reason.

Additional lifecycle scenarios (not a new classifier snapshot):

| Scenario                           | Terminal reason         | Retryable |
| ---------------------------------- | ----------------------- | --------- |
| `everyone-left-after-participants` | `everyone_left`         | no        |
| `silence-then-nobody-joined`       | `no_one_joined`         | yes       |
| `host-ended-after-long-capture`    | `meeting_ended`         | no        |
| `captcha-unsolved`                 | `authentication_failed` | no        |
| `error-page-before-join`           | `provider_error`        | no        |
| `stopped-by-request`               | `stopped_by_request`    | no        |
