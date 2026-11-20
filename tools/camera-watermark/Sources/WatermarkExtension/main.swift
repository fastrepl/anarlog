import CoreMediaIO
import Foundation

// The Camera Extension macOS loads on its own, as the `_cmiodalassistants` user, once the person
// allows it. It can't open the real camera, so Anarlog Camera.app feeds it through the sink stream.
let provider = ProviderSource()
CMIOExtensionProvider.startService(provider: provider.provider)
CFRunLoopRun()
