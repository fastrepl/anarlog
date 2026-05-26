import SwiftUI

enum FloatingBarLayout {
  static let inset: CGFloat = 4
  static let screenMargin: CGFloat = 8
  static let markSize: CGFloat = 20
  static let waveformWidth: CGFloat = 18
  static let waveformHeight: CGFloat = 13
  static let stopSquareSize: CGFloat = 9
  static let clickAreaSize: CGFloat = 28
  static let clickAreaGap: CGFloat = 0
  static let pillPadding: CGFloat = 2
  static let pillWidth: CGFloat = clickAreaSize + pillPadding * 2
  static let pillHeight: CGFloat = clickAreaSize * 2 + clickAreaGap + pillPadding * 2
  static let containerWidth: CGFloat = pillWidth + inset * 2
  static let containerHeight: CGFloat = pillHeight + inset * 2
  static let dragClickThreshold: CGFloat = 4
}

struct FloatingBarView: View {
  @ObservedObject var model: FloatingBarViewModel
  @State private var isBarsHovered = false
  @State private var suppressNextClick = false

  var body: some View {
    VStack(spacing: FloatingBarLayout.clickAreaGap) {
      Button(action: { performClick(RustBridge.openMainWindow) }) {
        CircularClickArea {
          Text("a")
            .font(.custom(FloatingBarFonts.cabinSketchName, size: FloatingBarLayout.markSize))
            .foregroundStyle(.white)
            .offset(y: -1)
        }
      }
      .buttonStyle(.plain)

      Button(action: { performClick(RustBridge.stopListening) }) {
        CircularClickArea(onHoverChange: { isBarsHovered = $0 }) {
          Group {
            if isBarsHovered {
              Rectangle()
                .fill(stopColor)
                .frame(
                  width: FloatingBarLayout.stopSquareSize,
                  height: FloatingBarLayout.stopSquareSize
                )
            } else {
              DancingBars(color: accentColor, amplitude: model.amplitude)
            }
          }
          .frame(
            width: FloatingBarLayout.waveformWidth,
            height: FloatingBarLayout.waveformHeight
          )
        }
      }
      .buttonStyle(.plain)
    }
    .padding(FloatingBarLayout.pillPadding)
    .frame(width: FloatingBarLayout.pillWidth, height: FloatingBarLayout.pillHeight)
    .contentShape(Capsule(style: .continuous))
    .simultaneousGesture(dragClickSuppressor)
    .background(
      Capsule(style: .continuous)
        .fill(Color(red: 0.43, green: 0.44, blue: 0.40).opacity(0.78))
    )
    .overlay(
      Capsule(style: .continuous)
        .strokeBorder(Color.white.opacity(0.14), lineWidth: 0.5)
    )
    .overlay(
      Capsule(style: .continuous)
        .strokeBorder(Color.white.opacity(0.28), lineWidth: 0.5)
        .padding(1.5)
    )
    .padding(FloatingBarLayout.inset)
  }

  private var accentColor: Color {
    model.degraded
      ? Color(red: 0.96, green: 0.62, blue: 0.04) : Color(red: 1, green: 0.2, blue: 0.23)
  }

  private var stopColor: Color {
    Color(red: 1, green: 0.2, blue: 0.23)
  }

  private var dragClickSuppressor: some Gesture {
    DragGesture(
      minimumDistance: FloatingBarLayout.dragClickThreshold,
      coordinateSpace: .global
    )
    .onChanged { _ in
      suppressNextClick = true
    }
    .onEnded { _ in
      DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
        suppressNextClick = false
      }
    }
  }

  private func performClick(_ action: () -> Void) {
    if suppressNextClick {
      suppressNextClick = false
      return
    }

    action()
  }
}

private struct CircularClickArea<Content: View>: View {
  private let content: () -> Content
  private let onHoverChange: (Bool) -> Void
  @State private var isHovered = false

  init(
    onHoverChange: @escaping (Bool) -> Void = { _ in },
    @ViewBuilder content: @escaping () -> Content
  ) {
    self.content = content
    self.onHoverChange = onHoverChange
  }

  var body: some View {
    content()
      .frame(
        width: FloatingBarLayout.clickAreaSize,
        height: FloatingBarLayout.clickAreaSize
      )
      .contentShape(Circle())
      .background(
        Circle()
          .fill(isHovered ? Color.white.opacity(0.08) : Color.clear)
      )
      .onHover { hovered in
        isHovered = hovered
        onHoverChange(hovered)
      }
  }
}

private struct DancingBars: View {
  let color: Color
  let amplitude: Double

  private let barCount = 3
  private let barWidth: CGFloat = 4
  private let barSpacing: CGFloat = 2
  private let minHeight: CGFloat = 2
  private let maxHeight: CGFloat = 13

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
    let drive = 0.4 + normalized * 0.9
    let height = maxHeight * CGFloat(drive * envelope * (0.4 + wave * 0.6))
    return max(minHeight, min(maxHeight, height))
  }
}
