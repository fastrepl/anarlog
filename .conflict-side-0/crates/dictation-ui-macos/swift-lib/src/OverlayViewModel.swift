import Combine
import Foundation

final class OverlayViewModel: ObservableObject {
  @Published var phase: Phase = .recording
  @Published var amplitude: Float = 0
}
