import Foundation

/// Delays a sheet's self-dismissal after a success confirmation. The caller
/// owns the task in view `@State` and cancels it when the sheet disappears, so
/// a timer from an earlier presentation can never dismiss a reopened sheet.
enum SheetAutoDismissSupport {
    static let delayNanoseconds: UInt64 = 1_000_000_000

    @MainActor
    static func schedule(
        _ dismissTask: inout Task<Void, Never>?,
        delayNanoseconds: UInt64 = delayNanoseconds,
        dismiss: @escaping @MainActor () -> Void
    ) {
        cancel(&dismissTask)
        dismissTask = Task { @MainActor in
            do {
                try await Task.sleep(nanoseconds: delayNanoseconds)
            } catch {
                return
            }

            guard !Task.isCancelled else {
                return
            }

            dismiss()
        }
    }

    @MainActor
    static func cancel(_ dismissTask: inout Task<Void, Never>?) {
        dismissTask?.cancel()
        dismissTask = nil
    }
}
