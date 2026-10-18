import Foundation
import Network

/// The app's one `NWPathMonitor`. Stores with an outbox observe it to sync
/// as soon as the network comes back.
@MainActor
final class NetworkMonitor {
    static let shared = NetworkMonitor()

    private(set) var isOnline = true
    private var observers: [@MainActor (Bool) -> Void] = []
    private let monitor = NWPathMonitor()

    init(startMonitoring: Bool = true) {
        guard startMonitoring else { return }
        monitor.pathUpdateHandler = { [weak self] path in
            Task { @MainActor in
                self?.pathChanged(isOnline: path.status == .satisfied)
            }
        }
        monitor.start(queue: DispatchQueue(label: "NetworkMonitor"))
    }

    deinit { monitor.cancel() }

    /// Calls `handler` with the new online state on every path update.
    func observe(_ handler: @escaping @MainActor (Bool) -> Void) {
        observers.append(handler)
    }

    func pathChanged(isOnline: Bool) {
        self.isOnline = isOnline
        for observer in observers {
            observer(isOnline)
        }
    }
}
