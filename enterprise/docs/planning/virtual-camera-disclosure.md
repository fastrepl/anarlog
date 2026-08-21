# Virtual camera disclosure

ANLG-134.

## Verdict

Not v1. Technically viable later on both Mac and Windows, with a high installer/signing cost and uneven meeting-app compatibility.

## macOS

System extension / Camera Extension (CoreMediaIO) can publish a virtual camera. Meeting apps that list cameras (Zoom, Meet via Chrome, Teams) generally can select it. Requires notarization, TCC camera permission, and a separate extension target. Overlaying a "Recording" chip on the outgoing feed is the useful disclosure UX. Replacing the real camera entirely is too invasive for enterprise defaults.

## Windows

A virtual camera driver (DirectShow / Media Foundation, or OBS-style) can do the same. Signing (EV cert + attestation) and enterprise deployment via Intune/MSI are the real cost. Teams and Zoom on Windows honor virtual cameras; some GPU capture paths do not.

## Product decision

v1 disclosure is chat/email/bot-visible participant (ANLG-135), not a virtual camera. Revisit after capture reliability and when a customer is blocked on visual disclosure specifically.
