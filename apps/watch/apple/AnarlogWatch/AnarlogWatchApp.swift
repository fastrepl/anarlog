import SwiftUI

@main
struct AnarlogWatchApp: App {
  @StateObject private var recorder = RecordingController()
  @StateObject private var syncController = WatchSyncController()

  var body: some Scene {
    WindowGroup {
      ContentView(
        recorder: recorder,
        syncController: syncController
      )
    }
  }
}
