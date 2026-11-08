import Foundation

/// Runs a store's sync passes one at a time. A request made while a pass
/// runs queues exactly one more pass after it, so an edit, reconnect or
/// server-side change that lands mid-sync is never dropped, and a burst of
/// them costs one extra pass, not one each.
@MainActor
final class SyncCoordinator {
    enum State: Equatable {
        case idle
        case syncing
        case syncingWithRerunQueued
    }

    private(set) var state: State = .idle {
        didSet {
            if (oldValue == .idle) != (state == .idle) {
                onBusyChanged?(state != .idle)
            }
        }
    }

    var isBusy: Bool { state != .idle }

    /// Called when the coordinator starts or stops running passes.
    var onBusyChanged: ((Bool) -> Void)?

    private let pass: () async -> Void
    private var running: Task<Void, Never>?

    init(pass: @escaping () async -> Void) {
        self.pass = pass
    }

    /// Ask for a pass without waiting for it: one starts now if idle, or
    /// once the running one ends.
    func request() {
        switch state {
        case .idle:
            state = .syncing
            running = Task { await drain() }
        case .syncing:
            state = .syncingWithRerunQueued
        case .syncingWithRerunQueued:
            break
        }
    }

    /// Ask for a pass and wait for it: returns once every pass, including
    /// one this call queued behind a running sync, has finished. Pull to
    /// refresh awaits this. Don't call it from inside a pass, which would
    /// wait on itself; use `request()` or `syncUnlessBusy()` there.
    func sync() async {
        request()
        await running?.value
    }

    /// Run a pass and wait for it when idle; when one is running, queue a
    /// rerun and return at once.
    func syncUnlessBusy() async {
        if isBusy {
            request()
        } else {
            await sync()
        }
    }

    private func drain() async {
        while true {
            await pass()
            guard state == .syncingWithRerunQueued else { break }
            state = .syncing
        }
        state = .idle
        running = nil
    }
}
