import SwiftUI

enum FloatingBarLayout {
  static let containerWidth: CGFloat = 124
  static let containerHeight: CGFloat = 48
  static let barWidth: CGFloat = 116
  static let barHeight: CGFloat = 40
  static let inset: CGFloat = 4
  static let screenMargin: CGFloat = 8
  static let markWidth: CGFloat = 24
  static let markSize: CGFloat = 31
  static let contentPaddingH: CGFloat = 6
  static let waveformWidth: CGFloat = 72
  static let waveformHeight: CGFloat = 20
  static let stopSquareSize: CGFloat = 8
}

struct FloatingBarView: View {
  @ObservedObject var model: FloatingBarViewModel
  @State private var isHoveringStop = false

  var body: some View {
    HStack(spacing: 8) {
      Button(action: RustBridge.openMainWindow) {
        Text("a")
          .font(.custom(FloatingBarFonts.cabinSketchName, size: FloatingBarLayout.markSize))
          .foregroundStyle(.white)
          .frame(width: FloatingBarLayout.markWidth, height: FloatingBarLayout.barHeight)
          .offset(y: -1)
      }
      .buttonStyle(.plain)

      Button(action: RustBridge.stopListening) {
        ZStack {
          DancingBars(color: accentColor, amplitude: model.amplitude)
            .opacity(isHoveringStop ? 0 : 1)

          HStack(spacing: 6) {
            Rectangle()
              .fill(accentColor)
              .frame(
                width: FloatingBarLayout.stopSquareSize,
                height: FloatingBarLayout.stopSquareSize)
            Text("Stop")
              .font(.system(size: 12, weight: .semibold))
              .foregroundStyle(.white.opacity(0.92))
          }
          .opacity(isHoveringStop ? 1 : 0)
        }
        .frame(
          width: FloatingBarLayout.waveformWidth,
          height: FloatingBarLayout.waveformHeight
        )
        .background(
          RoundedRectangle(cornerRadius: 6, style: .continuous)
            .fill(isHoveringStop ? Color.white.opacity(0.12) : Color.clear)
        )
      }
      .buttonStyle(.plain)
      .onHover { isHoveringStop = $0 }
    }
    .padding(.horizontal, FloatingBarLayout.contentPaddingH)
    .frame(width: FloatingBarLayout.barWidth, height: FloatingBarLayout.barHeight)
    .background(
      Capsule(style: .continuous)
        .fill(Color(red: 0.43, green: 0.44, blue: 0.40).opacity(0.78))
    )
    .overlay(
      Capsule(style: .continuous)
        .strokeBorder(Color.white.opacity(0.14), lineWidth: 0.5)
    )
    .padding(FloatingBarLayout.inset)
  }

  private var accentColor: Color {
    model.degraded
      ? Color(red: 0.96, green: 0.62, blue: 0.04) : Color(red: 1, green: 0.2, blue: 0.23)
  }
}

private struct DancingBars: View {
  let color: Color
  let amplitude: Double

  private let barCount = 15
  private let barWidth: CGFloat = 2.5
  private let barSpacing: CGFloat = 2
  private let minHeight: CGFloat = 3
  private let maxHeight: CGFloat = 18

  var body: some View {
    TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: false)) { timeline in
      HStack(spacing: barSpacing) {
        let t = timeline.date.timeIntervalSinceReferenceDate
        ForEach(0..<barCount, id: \.self) { index in
          Capsule(style: .continuous)
            .fill(color)
            .frame(width: barWidth, height: barHeight(index: index, time: t))
        }
      }
      .frame(maxHeight: .infinity, alignment: .center)
    }
  }

  private func barHeight(index: Int, time: TimeInterval) -> CGFloat {
    let normalized = min(max(amplitude, 0), 1)
    let center = Double(barCount - 1) / 2
    let distance = abs(Double(index) - center) / max(center, 1)
    let envelope = 1 - distance * 0.42
    let phase = time * 8.5 + Double(index) * 0.68
    let wave = sin(phase) * 0.5 + 0.5
    let drive = 0.24 + normalized * 0.76
    return max(minHeight, maxHeight * CGFloat(drive * envelope * (0.48 + wave * 0.52)))
  }
}
