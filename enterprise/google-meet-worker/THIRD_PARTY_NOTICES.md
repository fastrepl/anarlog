# Third-party notices

This component contains source-informed adaptations of Google Meet browser-launch and admission-classification behavior from Vexa `v0.12.18`, commit `1b62993e7e97c6ee04a5dcb116f7749ec74169df`.

Upstream: https://github.com/Vexa-ai/vexa

Reference modules:

- `core/meetings/modules/remote-browser/src/args.ts`
- `core/meetings/modules/join/src/browser-args.ts`
- `core/meetings/modules/join/src/googlemeet/admission.ts`
- `core/meetings/modules/join/src/googlemeet/humanized/humanizedInteraction.ts`
- `core/meetings/modules/join/src/googlemeet/humanized/x11Input.ts`
- `core/meetings/modules/join/src/googlemeet/join.ts`
- `core/meetings/modules/join/src/googlemeet/selectors.ts`

The Anarlog implementation was rewritten in Rust, reorganized around Anarlog's provider-neutral capture contract, and modified for direct Chromium DevTools ownership. Files carrying an adaptation notice have been changed by Fastrepl, Inc.

Vexa is licensed under the Apache License, Version 2.0. A copy is provided in `third-party/VEXA-LICENSE`.
