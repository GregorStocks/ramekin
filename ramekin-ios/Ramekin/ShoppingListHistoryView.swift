import SwiftUI

/// The history screen's state. Loading and undoing report failures
/// separately, so a reload that succeeds can't hide an undo that failed.
@MainActor
final class ShoppingListHistoryViewModel: ObservableObject {
    @Published private(set) var entries: [ShoppingListHistoryEntry] = []
    @Published private(set) var isLoading = true
    @Published private(set) var loadError: String?
    @Published private(set) var undoError: String?
    @Published private(set) var undoingId: UUID?

    private let loadHistory: () async throws -> [ShoppingListHistoryEntry]
    private let undoEntry: (UUID) async throws -> Void
    private var loadGeneration = UUID()

    init(
        loadHistory: @escaping () async throws -> [ShoppingListHistoryEntry],
        undoEntry: @escaping (UUID) async throws -> Void
    ) {
        self.loadHistory = loadHistory
        self.undoEntry = undoEntry
    }

    convenience init(store: ShoppingListStore) {
        self.init(
            loadHistory: { try await store.loadHistory() },
            undoEntry: { try await store.undo(entry: $0) }
        )
    }

    func load() async {
        let generation = UUID()
        loadGeneration = generation
        isLoading = true
        do {
            let loaded = try await loadHistory()
            guard loadGeneration == generation else { return }
            entries = loaded
            loadError = nil
        } catch {
            guard loadGeneration == generation else { return }
            if !CancellationSupport.isCancellation(error) && !Task.isCancelled {
                loadError = "Couldn't load history: \(error.localizedDescription)"
            }
        }
        isLoading = false
    }

    /// History lives on the server, so the reload after an undo shows it
    /// even while the local list is still syncing the result.
    func undo(_ entry: ShoppingListHistoryEntry) async {
        guard undoingId == nil else { return }
        undoingId = entry.id
        undoError = nil
        defer { undoingId = nil }
        do {
            try await undoEntry(entry.id)
        } catch {
            if !CancellationSupport.isCancellation(error) {
                undoError = "Couldn't undo: \(error.localizedDescription)"
            }
            return
        }
        await load()
    }
}

/// Every recent change to the shopping list, newest first, each undoable
/// until something undoes it. Online only: history lives on the server.
struct ShoppingListHistoryView: View {
    @StateObject private var model: ShoppingListHistoryViewModel

    init(store: ShoppingListStore) {
        _model = StateObject(wrappedValue: ShoppingListHistoryViewModel(store: store))
    }

    var body: some View {
        Group {
            if model.isLoading && model.entries.isEmpty {
                ProgressView()
            } else if model.entries.isEmpty && model.loadError == nil && model.undoError == nil {
                Text("No changes yet")
                    .foregroundColor(.secondary)
            } else {
                List {
                    ForEach([model.undoError, model.loadError].compactMap { $0 }, id: \.self) { message in
                        Text(message)
                            .font(.footnote)
                            .foregroundColor(.red)
                    }
                    ForEach(model.entries, id: \.id) { entry in
                        row(entry)
                    }
                }
                .listStyle(.plain)
            }
        }
        .navigationTitle("History")
        .navigationBarTitleDisplayMode(.inline)
        .task { await model.load() }
        .refreshable { await model.load() }
    }

    private func row(_ entry: ShoppingListHistoryEntry) -> some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(entry.summary)
                    .foregroundColor(entry.undoneById == nil ? .primary : .secondary)
                Text(entry.createdAt.formatted(.relative(presentation: .named)))
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            Spacer()
            if entry.undoneById != nil {
                Text("Undone")
                    .font(.caption)
                    .foregroundColor(.secondary)
            } else {
                Button("Undo") {
                    Task { await model.undo(entry) }
                }
                .buttonStyle(.borderless)
                .disabled(model.undoingId != nil)
            }
        }
    }
}
