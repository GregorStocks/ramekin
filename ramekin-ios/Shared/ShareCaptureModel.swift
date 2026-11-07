import Foundation

/// State machine behind the share extension's sheet: one capture attempt at a
/// time, a "still working" affordance that belongs to the attempt that
/// scheduled it, and an extension request that finishes exactly once.
/// Lives in Shared so the app's unit tests can drive it.
@MainActor
final class ShareCaptureModel: ObservableObject {
    enum Status: Equatable {
        case ready
        /// `slow` once the attempt has run past `slowAffordanceDelay`.
        case sending(slow: Bool)
        case success
        case failed(String)
        case notLoggedIn
    }

    typealias Capture = (SharedPagePayload) async throws -> Void
    typealias Sleep = (TimeInterval) async throws -> Void

    /// Delay after which, if the attempt is still sending, the sheet offers
    /// "Still working, tap to close" so the user can dismiss instead of
    /// waiting for iOS to terminate the extension.
    static let slowAffordanceDelay: TimeInterval = 10
    /// How long "Saved!" shows before the sheet closes itself.
    static let successDismissDelay: TimeInterval = 1.5

    static let missingPayloadMessage =
        "Could not read page content. Open the page in Safari and share from there."
    static let forbiddenMessage =
        "The server refused the request (HTTP 403). If you use Cloudflare Access, "
        + "check the service token in the Ramekin app's sign-in settings."

    @Published private(set) var status: Status = .ready

    private let payload: SharedPagePayload?
    private let isLoggedIn: () -> Bool
    private let capture: Capture
    private let sleep: Sleep
    private let onComplete: () -> Void
    private let onCancel: () -> Void

    private var started = false
    private var finished = false
    private var attemptCount = 0
    /// The in-flight attempt. Readable so tests can await it.
    private(set) var sendTask: Task<Void, Never>?

    init(
        payload: SharedPagePayload?,
        isLoggedIn: @escaping () -> Bool = { RamekinAPI.shared.isLoggedIn },
        capture: @escaping Capture = ShareCaptureModel.captureWithRamekinAPI,
        sleep: @escaping Sleep = { try await Task.sleep(nanoseconds: UInt64($0 * 1_000_000_000)) },
        onComplete: @escaping () -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.payload = payload
        self.isLoggedIn = isLoggedIn
        self.capture = capture
        self.sleep = sleep
        self.onComplete = onComplete
        self.onCancel = onCancel
    }

    /// Begin the capture. Only the first call does anything, so a repeated
    /// `onAppear` can't POST the page twice.
    func start() {
        guard !started else { return }
        started = true
        beginAttempt()
    }

    /// Try again after a failure. Ignored in any other state.
    func retry() {
        guard case .failed = status, !finished else { return }
        beginAttempt()
    }

    /// Finish the extension request successfully (Done, "Still working", or
    /// the automatic close after success). Only the first finish counts. A
    /// slow upload is left running: it may still land before iOS tears the
    /// extension down.
    func complete() {
        guard finish() else { return }
        onComplete()
    }

    /// Cancel the extension request (Cancel or OK) and any upload in flight.
    /// Only the first finish counts.
    func cancel() {
        guard finish() else { return }
        sendTask?.cancel()
        onCancel()
    }

    private func finish() -> Bool {
        guard !finished else { return false }
        finished = true
        return true
    }

    private func beginAttempt() {
        DebugLogger.shared.log("Share attempt; isLoggedIn: \(isLoggedIn())", source: "ShareExtension")
        guard isLoggedIn() else {
            status = .notLoggedIn
            return
        }
        guard let payload else {
            DebugLogger.shared.log("ERROR: No payload from Safari preprocessing", source: "ShareExtension")
            status = .failed(Self.missingPayloadMessage)
            return
        }
        send(payload)
    }

    private func send(_ payload: SharedPagePayload) {
        sendTask?.cancel()
        attemptCount += 1
        let thisAttempt = attemptCount
        status = .sending(slow: false)

        sendTask = Task { [weak self] in
            guard let self else { return }
            let slowTimer = Task { [weak self] in
                guard let self else { return }
                try? await self.sleep(Self.slowAffordanceDelay)
                guard !Task.isCancelled, self.attemptCount == thisAttempt,
                      self.status == .sending(slow: false) else { return }
                DebugLogger.shared.log(
                    "Still sending after \(Int(Self.slowAffordanceDelay))s - showing slow affordance",
                    source: "ShareExtension"
                )
                self.status = .sending(slow: true)
            }
            defer { slowTimer.cancel() }

            do {
                try await self.capture(payload)
                guard !Task.isCancelled, self.attemptCount == thisAttempt else { return }
                DebugLogger.shared.log("Capture succeeded", source: "ShareExtension")
                self.status = .success
                slowTimer.cancel()
                try? await self.sleep(Self.successDismissDelay)
                guard !Task.isCancelled else { return }
                self.complete()
            } catch {
                guard !Task.isCancelled, self.attemptCount == thisAttempt else { return }
                DebugLogger.shared.log("Capture FAILED: \(error)", source: "ShareExtension")
                self.status = Self.outcome(for: error)
            }
        }
    }

    /// The status a failed attempt ends in. Only a missing or rejected (401)
    /// session reads as signed out, matching how the app treats a 401; a 403
    /// is a refusal signing in again wouldn't fix, so it stays retryable.
    static func outcome(for error: Error) -> Status {
        guard let apiError = error as? RamekinAPI.APIError else {
            return .failed(error.localizedDescription)
        }
        switch apiError {
        case .noAuthToken:
            return .notLoggedIn
        case .httpError(let statusCode, _, _) where AuthExpiry.isSessionRejection(statusCode: statusCode):
            return .notLoggedIn
        case .httpError(403, _, _):
            return .failed(forbiddenMessage)
        default:
            return .failed(apiError.localizedDescription)
        }
    }

    /// Check the session, then upload the page.
    nonisolated static func captureWithRamekinAPI(_ payload: SharedPagePayload) async throws {
        // Fail fast on an expired/invalid token before uploading the page. A
        // doomed upload can otherwise hang on a buffering proxy.
        try await DebugLogger.shared.timed("verifyAuth", source: "ShareExtension") {
            try await RamekinAPI.shared.verifyAuthenticated()
        }
        _ = try await DebugLogger.shared.timed("captureHTML", source: "ShareExtension") {
            try await RamekinAPI.shared.captureHTML(
                html: payload.html,
                sourceURL: payload.url.absoluteString
            )
        }
    }
}
