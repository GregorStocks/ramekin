import SwiftUI

/// Every recent change to the shopping list, newest first, each undoable
/// until something undoes it. Online only: history lives on the server.
struct ShoppingListHistoryView: View {
    @ObservedObject var store: ShoppingListStore
    @State private var entries: [ShoppingListHistoryEntry] = []
    @State private var isLoading = true
    @State private var errorMessage: String?
    @State private var undoingId: UUID?

    var body: some View {
        Group {
            if isLoading && entries.isEmpty {
                ProgressView()
            } else if entries.isEmpty && errorMessage == nil {
                Text("No changes yet")
                    .foregroundColor(.secondary)
            } else {
                List {
                    if let errorMessage {
                        Text(errorMessage)
                            .font(.footnote)
                            .foregroundColor(.red)
                    }
                    ForEach(entries, id: \.id) { entry in
                        row(entry)
                    }
                }
                .listStyle(.plain)
            }
        }
        .navigationTitle("History")
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
        .refreshable { await load() }
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
                    Task { await undo(entry) }
                }
                .buttonStyle(.borderless)
                .disabled(undoingId != nil)
            }
        }
    }

    private func load() async {
        isLoading = true
        defer { isLoading = false }
        do {
            entries = try await store.loadHistory()
            errorMessage = nil
        } catch {
            errorMessage = "Couldn't load history: \(error.localizedDescription)"
        }
    }

    private func undo(_ entry: ShoppingListHistoryEntry) async {
        undoingId = entry.id
        defer { undoingId = nil }
        do {
            try await store.undo(entry: entry.id)
            await load()
        } catch {
            errorMessage = "Couldn't undo: \(error.localizedDescription)"
        }
    }
}
