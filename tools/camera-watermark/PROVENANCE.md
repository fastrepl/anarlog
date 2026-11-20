# Provenance

On 2026-11-19, Fastrepl authorized distribution of its Char code ported here under Anarlog's MIT license.

| Field | Value |
| --- | --- |
| Upstream | Char monorepo (Fastrepl, Inc.), `tools/camera-watermark` and `crates/desktop-agent-automation/src/meeting_camera` |
| Revision | `d4608723d8ac328ab91091803a08c4585a0dc244` (contains watermark branch head `95b5d6723948bb7f53e042413b61d1a4f5a4e567`) |
| Copyright | Char Copyright (C) 2023-present Fastrepl, Inc. |
| License | GNU General Public License v3 |
| Use | Copied and modified |

## Copied and modified

Swift, under `tools/camera-watermark/`:

- `Sources/WatermarkExtension/{main,Provider}.swift`: identities, logging, and the sink gate now
  trusts the extension's own signing team instead of a hard-coded one.
- `Sources/WatermarkHelper/{main,AgentService,Activation,Sink,CameraFeed,CameraBench}.swift`: Anarlog
  names. `serve` drops style previews. `render` drops the stand-in clip and `--loop`. The camera picker
  never wraps a virtual camera.
- `Sources/WatermarkKit/{AgentState,BackgroundScene,CameraIdentity,Compositor,FeedState,HelperProtocol,PersonSegmenter,ProtectedOutput,SVGPath,SinkGate,Watermark,WatermarkBackground,WatermarkMotion,WatermarkStyle}.swift`:
  Anarlog identities, state folder and wordmark. Preview and loop encoding were removed.
- `Tests/WatermarkKitTests/WatermarkKitTests.swift`: ported, minus stand-in and preview tests, plus
  identity, coexistence and team-gate tests.
- `scripts/bundle.mjs`: Anarlog identities, build-time `--team-id`, explicit signing limits.

Rust: `crates/detect/src/meeting_camera/{mod,matching,macos}.rs` from
`crates/desktop-agent-automation/src/meeting_camera/`, using the notices lane's shared
`meeting_automation` helpers and lock.

Behavioral reference only: `char/apps/desktop/src/main/camera-watermark/{helper,protocol,auto-switch,auto-switch-host,reaction,setup}.ts`,
reimplemented in Rust in `plugins/camera-watermark`.

## Not imported

Char's wordmark, settings UI, preset gallery, Storybook renders, and the stand-in clip
(`Resources/stand-in.mp4`), plus `scripts/render-styles.mjs`. The wordmark drawn here is Anarlog's
own, from `apps/web/public/logo.svg`.
