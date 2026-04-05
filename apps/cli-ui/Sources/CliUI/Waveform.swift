import SwiftUI

struct WaveformView: View {
  let level: Float
  let tick: UInt64

  private let barCount = 9
  @State private var animatedBars: [CGFloat] = Array(repeating: 0.15, count: 9)

  var body: some View {
    HStack(spacing: 2) {
      ForEach(0..<barCount, id: \.self) { index in
        RoundedRectangle(cornerRadius: 1)
          .fill(Color.white.opacity(0.9))
          .frame(width: 2.5, height: animatedBars[index] * 18)
      }
    }
    .frame(height: 18)
    .onChange(of: tick) { _, _ in
      withAnimation(.easeInOut(duration: 0.1)) {
        for i in 0..<barCount {
          let centerDistance = abs(Float(i) - Float(barCount / 2)) / Float(barCount / 2)
          let variation = Float.random(in: 0.7...1.3)
          let height = max(0.15, level * (1.0 - centerDistance * 0.4) * variation)
          animatedBars[i] = CGFloat(min(1.0, height))
        }
      }
    }
  }
}
