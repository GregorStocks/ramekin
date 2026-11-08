import XCTest
@testable import Ramekin

@MainActor
final class SyncCoordinatorTests: XCTestCase {
    /// A pass that waits until the test lets it finish.
    @MainActor
    private final class GatedPass {
        private(set) var started = 0
        private var gates: [CheckedContinuation<Void, Never>] = []
        private var startWaiters: [(count: Int, continuation: CheckedContinuation<Void, Never>)] = []

        func run() async {
            started += 1
            let ready = startWaiters.filter { $0.count <= started }
            startWaiters.removeAll { $0.count <= started }
            ready.forEach { $0.continuation.resume() }
            await withCheckedContinuation { gates.append($0) }
        }

        func waitForStart(_ count: Int) async {
            guard started < count else { return }
            await withCheckedContinuation { startWaiters.append((count, $0)) }
        }

        func finishOne() {
            gates.removeFirst().resume()
        }
    }

    func testRequestsDuringAPassQueueExactlyOneRerun() async {
        let pass = GatedPass()
        let coordinator = SyncCoordinator { await pass.run() }

        let first = Task { await coordinator.sync() }
        await pass.waitForStart(1)
        coordinator.request()
        coordinator.request()
        coordinator.request()
        XCTAssertEqual(coordinator.state, .syncingWithRerunQueued)

        pass.finishOne()
        await pass.waitForStart(2)
        XCTAssertEqual(coordinator.state, .syncing)
        pass.finishOne()
        await first.value

        XCTAssertEqual(pass.started, 2)
        XCTAssertEqual(coordinator.state, .idle)
    }

    func testSyncDuringAPassWaitsForTheRerun() async {
        let pass = GatedPass()
        let coordinator = SyncCoordinator { await pass.run() }
        var refreshed = false

        coordinator.request()
        await pass.waitForStart(1)
        let refresh = Task {
            await coordinator.sync()
            refreshed = true
        }
        pass.finishOne()
        await pass.waitForStart(2)
        XCTAssertFalse(refreshed, "returned before the pass it queued ran")

        pass.finishOne()
        await refresh.value
        XCTAssertTrue(refreshed)
        XCTAssertEqual(pass.started, 2)
    }

    func testSyncUnlessBusyQueuesWithoutWaitingDuringAPass() async {
        let pass = GatedPass()
        let coordinator = SyncCoordinator { await pass.run() }

        let first = Task { await coordinator.sync() }
        await pass.waitForStart(1)
        // Returns at once rather than waiting on the gated pass.
        await coordinator.syncUnlessBusy()
        XCTAssertEqual(coordinator.state, .syncingWithRerunQueued)

        pass.finishOne()
        await pass.waitForStart(2)
        pass.finishOne()
        await first.value
        XCTAssertEqual(pass.started, 2)
    }

    func testBusyChangesOnlyWhenPassesStartAndStop() async {
        var changes: [Bool] = []
        var passes = 0
        var coordinator: SyncCoordinator!
        coordinator = SyncCoordinator {
            passes += 1
            // Requested mid-pass: one rerun, with no idle blip between.
            if passes == 1 { coordinator.request() }
        }
        coordinator.onBusyChanged = { changes.append($0) }

        await coordinator.sync()

        XCTAssertEqual(passes, 2)
        XCTAssertEqual(changes, [true, false])
        XCTAssertFalse(coordinator.isBusy)
    }
}
