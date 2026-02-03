import SwiftUI

struct ShoppingListView: View {
    @StateObject private var store = ShoppingListStore.shared

    /// Category display order for grouping
    private static let categoryOrder = [
        "Produce",
        "Meat & Seafood",
        "Dairy & Eggs",
        "Cheese",
        "Bakery & Bread",
        "Frozen",
        "Pasta & Rice",
        "Canned Goods",
        "Baking",
        "Spices & Seasonings",
        "Condiments & Sauces",
        "Oils & Vinegars",
        "Nuts & Dried Fruit",
        "Beverages",
        "Snacks",
        "Other"
    ]

    var body: some View {
        NavigationStack {
            Group {
                if store.items.isEmpty {
                    emptyState
                } else {
                    itemsList
                }
            }
            .navigationTitle("Shopping List")
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    if store.items.contains(where: \.isChecked) {
                        Button("Clear Checked") {
                            store.clearChecked()
                        }
                    }
                }
            }
            .refreshable {
                await store.syncWithServer()
            }
            .overlay(alignment: .top) {
                if !store.isOnline {
                    offlineBanner
                }
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 16) {
            Image(systemName: "cart")
                .font(.system(size: 60))
                .foregroundColor(.secondary)
            Text("Your shopping list is empty")
                .font(.headline)
                .foregroundColor(.secondary)
            Text("Add ingredients from a recipe to get started")
                .font(.subheadline)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding()
    }

    /// Groups unchecked items by category in display order
    private var groupedUncheckedItems: [(category: String, items: [ShoppingItem])] {
        let unchecked = store.items.filter { !$0.isChecked }
        let grouped = Dictionary(grouping: unchecked) { $0.category ?? "Other" }

        return Self.categoryOrder.compactMap { category in
            guard let items = grouped[category], !items.isEmpty else { return nil }
            return (category: category, items: items)
        }
    }

    private var itemsList: some View {
        List {
            let checked = store.items.filter(\.isChecked)

            // Unchecked items grouped by category
            ForEach(groupedUncheckedItems, id: \.category) { group in
                Section(group.category) {
                    ForEach(group.items, id: \.id) { item in
                        ShoppingItemRow(item: item, store: store)
                    }
                    .onDelete { offsets in
                        for offset in offsets {
                            store.deleteItem(group.items[offset])
                        }
                    }
                }
            }

            // Checked items in a separate section at the bottom
            if !checked.isEmpty {
                Section("Checked") {
                    ForEach(checked, id: \.id) { item in
                        ShoppingItemRow(item: item, store: store)
                    }
                    .onDelete { offsets in
                        for offset in offsets {
                            store.deleteItem(checked[offset])
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
    }

    private var offlineBanner: some View {
        HStack {
            Image(systemName: "wifi.slash")
            Text("Offline - changes will sync when connected")
        }
        .font(.caption)
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(Color.orange)
        .foregroundColor(.white)
        .clipShape(Capsule())
        .padding(.top, 8)
    }
}

struct ShoppingItemRow: View {
    let item: ShoppingItem
    let store: ShoppingListStore

    var body: some View {
        Button {
            store.toggleChecked(item)
        } label: {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: item.isChecked ? "checkmark.circle.fill" : "circle")
                    .font(.title2)
                    .foregroundColor(item.isChecked ? .green : .secondary)

                VStack(alignment: .leading, spacing: 2) {
                    Text(item.item ?? "")
                        .font(.body)
                        .strikethrough(item.isChecked)
                        .foregroundColor(item.isChecked ? .secondary : .primary)

                    if let amount = item.amount, !amount.isEmpty {
                        Text(amount)
                            .font(.subheadline)
                            .foregroundColor(.secondary)
                    }

                    if let recipeTitle = item.sourceRecipeTitle {
                        Text("from \(recipeTitle)")
                            .font(.caption)
                            .foregroundColor(.orange)
                    }
                }

                Spacer()
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

#Preview {
    ShoppingListView()
}
