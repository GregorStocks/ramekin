import Foundation

/// Where a recipe form stands relative to an unsaved draft from before the
/// app was terminated: the draft is offered on a banner until the user
/// resumes it, discards it, or opens a fresh form instead.
enum RecipeFormDraftFlow: Equatable {
    case closed
    case offered(RecipeFormDraft)
    /// The form is open, resuming `resuming` or starting fresh when nil.
    case open(resuming: RecipeFormDraft?)

    var offeredDraft: RecipeFormDraft? {
        if case let .offered(draft) = self { return draft }
        return nil
    }

    var isOpen: Bool {
        if case .open = self { return true }
        return false
    }

    /// The draft an open form starts from; nil for a fresh form.
    var resumingDraft: RecipeFormDraft? {
        if case let .open(draft) = self { return draft }
        return nil
    }

    /// Offers the stored draft, if any. An open form keeps going: its own
    /// edits are newer than anything stored.
    mutating func offer(_ draft: RecipeFormDraft?) {
        guard !isOpen else { return }
        self = draft.map(Self.offered) ?? .closed
    }

    /// Opens an empty form. A fresh form replaces the offered draft.
    mutating func openFresh() {
        self = .open(resuming: nil)
    }

    mutating func resume() {
        guard case let .offered(draft) = self else { return }
        self = .open(resuming: draft)
    }

    mutating func discard() {
        guard case .offered = self else { return }
        self = .closed
    }

    mutating func close() {
        guard isOpen else { return }
        self = .closed
    }
}
