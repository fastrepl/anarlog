import AppKit
import SwiftUI

let app = NSApplication.shared
app.setActivationPolicy(.accessory)

let state = CliUIState()

let hostingView = NSHostingView(rootView: ContentView(state: state))
hostingView.frame = NSRect(x: 0, y: 0, width: 220, height: 44)

let panel = OverlayPanel(contentView: hostingView)
panel.orderFrontRegardless()

// Read JSON lines from stdin on a background thread
let stdinThread = Thread {
  while let line = readLine() {
    guard let message = InboundMessage.parse(line) else { continue }

    DispatchQueue.main.async {
      switch message {
      case .state(let msg):
        if let recording = msg.recording {
          state.isRecording = recording
        }
        state.status = msg.status
      case .levels(let msg):
        state.audioLevel = msg.left
        state.levelTick &+= 1
      case .dismiss:
        app.terminate(nil)
      }
    }
  }
  // stdin closed — daemon died or told us to quit
  DispatchQueue.main.async {
    app.terminate(nil)
  }
}
stdinThread.start()

app.run()
