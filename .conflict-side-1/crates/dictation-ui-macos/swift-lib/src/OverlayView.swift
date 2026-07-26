import SwiftUI

struct OverlayView: View {
  @ObservedObject var model: OverlayViewModel

  var body: some View {
    HStack(spacing: OverlayLayout.iconBarsGap) {
      Image(systemName: iconName)
        .font(.system(size: OverlayLayout.iconSize, weight: .semibold))
        .foregroundStyle(iconColor)
        .frame(width: OverlayLayout.iconSize + 2, height: OverlayLayout.iconSize + 2)

      WaveformBars(amplitude: model.amplitude, phase: model.phase)
        .frame(width: barsWidth, height: OverlayLayout.barMaxHeight)
    }
    .padding(.horizontal, OverlayLayout.contentPaddingH)
    .frame(width: OverlayLayout.panelWidth, height: OverlayLayout.panelHeight)
    .background(
      Capsule(style: .continuous)
        .fill(
          LinearGradient(
            colors: [
              Color(white: 0.14),
              Color(white: 0.08),
            ],
            startPoint: .top,
            endPoint: .bottom
          )
        )
    )
    .overlay(
      Capsule(style: .continuous)
        .strokeBorder(Color.white.opacity(0.09), lineWidth: 0.5)
    )
    .shadow(color: glowColor.opacity(Double(model.amplitude) * 0.5), radius: 10, y: 0)
    .shadow(color: .black.opacity(0.38), radius: 8, y: 4)
    .padding(OverlayLayout.shadowPadding)
  }

  private var iconName: String {
    switch model.phase {
    case .recording: return "mic.fill"
    case .processing: return "waveform"
    }
  }

  private var iconColor: Color {
    switch model.phase {
    case .recording: return Color(red: 1.0, green: 0.28, blue: 0.28)
    case .processing: return Color(white: 0.72)
    }
  }

  private var glowColor: Color {
    switch model.phase {
    case .recording: return Color(red: 1.0, green: 0.2, blue: 0.2)
    case .processing: return .clear
    }
  }

  private var barsWidth: CGFloat {
    CGFloat(OverlayLayout.barCount) * OverlayLayout.barWidth
      + CGFloat(max(OverlayLayout.barCount - 1, 0)) * OverlayLayout.barSpacing
  }
}

private struct WaveformBars: View {
  let amplitude: Float
  let phase: Phase

  var body: some View {
    TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: false)) { timeline in
      let t = timeline.date.timeIntervalSinceReferenceDate
      HStack(spacing: OverlayLayout.barSpacing) {
        ForEach(0..<OverlayLayout.barCount, id: \.self) { i in
          Capsule(style: .continuous)
            .fill(fill(index: i))
            .frame(width: OverlayLayout.barWidth, height: height(for: i, t: t))
        }
      }
      .frame(maxHeight: .infinity, alignment: .center)
    }
  }

  private func fill(index: Int) -> Color {
    switch phase {
    case .recording:
      let mid = Double(OverlayLayout.barCount - 1) / 2.0
      let dist = abs(Double(index) - mid) / mid
      let brightness = 1.0 - dist * 0.2
      return Color(red: 1.0 * brightness, green: 0.28 * brightness, blue: 0.28 * brightness)
    case .processing:
      return Color(white: 0.55)
    }
  }

  private func height(for index: Int, t: TimeInterval) -> CGFloat {
    let normalized = CGFloat(min(max(amplitude, 0), 1))
    let mid = Double(OverlayLayout.barCount - 1) / 2.0
    let dist = abs(Double(index) - mid) / mid
    let envelope = CGFloat(1.0 - dist * 0.45)

    let phaseOffset = Double(index) * 0.55
    let freq = phase == .processing ? 3.5 : 9.0
    let wave = CGFloat(sin(t * freq + phaseOffset) * 0.5 + 0.5)

    let idleFloor: CGFloat = phase == .processing ? 0.25 : 0.22
    let drive = idleFloor + normalized * (1.0 - idleFloor)
    let scale = drive * envelope * (0.45 + 0.55 * wave)
    return max(OverlayLayout.barMinHeight, OverlayLayout.barMaxHeight * scale)
  }
}
