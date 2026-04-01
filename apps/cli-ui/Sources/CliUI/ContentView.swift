import SwiftUI

struct ContentView: View {
  @ObservedObject var state: CliUIState

  var body: some View {
    HStack(spacing: 10) {
      // Cancel button
      Button(action: { sendAction("cancel") }) {
        Image(systemName: "xmark")
          .font(.system(size: 10, weight: .medium))
          .foregroundColor(.white.opacity(0.6))
          .frame(width: 24, height: 24)
          .background(Color.white.opacity(0.1))
          .clipShape(Circle())
      }
      .buttonStyle(.plain)

      // Recording indicator + waveform
      HStack(spacing: 6) {
        Circle()
          .fill(state.isRecording ? Color.blue : Color.gray)
          .frame(width: 8, height: 8)

        WaveformView(level: state.audioLevel, tick: state.levelTick)
      }

      // Stop button
      Button(action: { sendAction("stop") }) {
        RoundedRectangle(cornerRadius: 2)
          .fill(Color.red)
          .frame(width: 10, height: 10)
          .frame(width: 24, height: 24)
          .background(Color.white.opacity(0.1))
          .clipShape(Circle())
      }
      .buttonStyle(.plain)
    }
    .padding(.horizontal, 12)
    .padding(.vertical, 8)
    .background(
      RoundedRectangle(cornerRadius: 26)
        .fill(.ultraThinMaterial)
        .environment(\.colorScheme, .dark)
    )
    .overlay(
      RoundedRectangle(cornerRadius: 26)
        .stroke(Color.white.opacity(0.1), lineWidth: 0.5)
    )
  }
}

final class CliUIState: ObservableObject {
  @Published var isRecording = true
  @Published var audioLevel: Float = 0
  @Published var levelTick: UInt64 = 0
  @Published var status: String?
}
