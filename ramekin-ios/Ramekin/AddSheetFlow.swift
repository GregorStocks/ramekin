import Foundation
import UIKit

/// The "add to meal plan / shopping list" sheets' shared flow:
/// idle -> confirmed (then dismisses itself) | failed. Both adds save
/// offline and return at once, so there is no submitting state to show.
/// The sheet owns one as a `@StateObject` and calls `cancel()` when it
/// disappears, so a dismissal from one presentation can't close the next.
@MainActor
final class AddSheetFlow: ObservableObject {
    enum Phase: Equatable {
        case idle
        case failed(String)
        /// Holds the confirmation as worded at submit time, so it doesn't
        /// change under the overlay.
        case confirmed(String)
    }

    @Published private(set) var phase: Phase = .idle

    private let delayNanoseconds: UInt64
    private let announce: @MainActor (String) -> Void
    private var dismissTask: Task<Void, Never>?

    init(
        delayNanoseconds: UInt64 = SheetAutoDismissSupport.delayNanoseconds,
        announce: @escaping @MainActor (String) -> Void = {
            UIAccessibility.post(notification: .announcement, argument: $0)
        }
    ) {
        self.delayNanoseconds = delayNanoseconds
        self.announce = announce
    }

    var isConfirmed: Bool {
        if case .confirmed = phase { return true }
        return false
    }

    var error: String? {
        if case .failed(let message) = phase { return message }
        return nil
    }

    var confirmation: String? {
        if case .confirmed(let message) = phase { return message }
        return nil
    }

    /// Run `add`. On success show and announce `confirmation`, then call
    /// `dismiss` after a beat; on failure show why. Ignored once confirmed,
    /// so a second tap can't add twice.
    func submit(
        confirmation: String,
        dismiss: @escaping @MainActor () -> Void,
        add: () throws -> Void
    ) {
        guard !isConfirmed else { return }
        do {
            try add()
        } catch {
            phase = .failed(error.localizedDescription)
            return
        }
        phase = .confirmed(confirmation)
        announce(confirmation)
        SheetAutoDismissSupport.schedule(&dismissTask, delayNanoseconds: delayNanoseconds, dismiss: dismiss)
    }

    /// Drop a pending self-dismissal: the sheet was cancelled or swiped away.
    func cancel() {
        SheetAutoDismissSupport.cancel(&dismissTask)
    }
}
