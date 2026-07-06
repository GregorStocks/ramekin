import SwiftUI
import UIKit

struct ShoppingListView: View {
    @ObservedObject private var store = ShoppingListStore.shared
    @State private var isAddingItem = false
    @State private var ingredientName = ""
    @State private var amount = ""
    @State private var addedCount = 0
    @State private var addTapTime: CFAbsoluteTime?
    @FocusState private var addFieldFocused: Bool

    var body: some View {
        NavigationStack {
            Group {
                if store.items.isEmpty && !isAddingItem {
                    emptyState
                } else {
                    itemsList
                }
            }
            .navigationTitle("Shopping List")
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    HStack(spacing: 12) {
                        Button {
                            addTapTime = CFAbsoluteTimeGetCurrent()
                            DebugLogger.shared.log("add tapped", source: "Shopping")
                            if isAddingItem {
                                addFieldFocused = true
                            } else {
                                isAddingItem = true
                            }
                        } label: {
                            Image(systemName: "plus")
                        }
                        if store.items.contains(where: \.isChecked) {
                            Button("Clear Checked") {
                                store.clearChecked()
                            }
                        }
                    }
                }
            }
            .onChange(of: isAddingItem) { adding in
                if !adding {
                    addedCount = 0
                }
            }
            .refreshable {
                await store.syncWithServer()
            }
            .onReceive(
                NotificationCenter.default.publisher(
                    for: UIResponder.keyboardDidShowNotification
                )
            ) { _ in
                if let tapTime = addTapTime {
                    let elapsedMs = Int((CFAbsoluteTimeGetCurrent() - tapTime) * 1000)
                    DebugLogger.shared.log(
                        "keyboard shown +\(elapsedMs)ms after add tap",
                        source: "Shopping"
                    )
                    addTapTime = nil
                }
            }
            .overlay(alignment: .top) {
                if !store.isOnline {
                    offlineBanner
                }
            }
            .overlay(alignment: .bottom) {
                if addedCount > 0 {
                    HStack {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(.green)
                        Text(addedCount == 1 ? "1 item added" : "\(addedCount) items added")
                    }
                    .font(.subheadline.weight(.medium))
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .background(.regularMaterial)
                    .clipShape(Capsule())
                    .padding(.bottom, 8)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
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
            Text("Add ingredients from a recipe or tap + to add manually")
                .font(.subheadline)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding()
    }

    /// Groups unchecked items by category in the server-provided display order
    private var groupedUncheckedItems: [(category: String, items: [ShoppingItem])] {
        let unchecked = store.items.filter { !$0.isChecked }
        let grouped = Dictionary(grouping: unchecked) { $0.category ?? "Other" }
        let ordered = ShoppingListGroupingSupport.orderedCategories(
            present: Set(grouped.keys),
            categoryOrder: store.categoryOrder
        )

        // orderedCategories only returns categories present in `grouped`
        return ordered.map { (category: $0, items: grouped[$0]!) }
    }

    private var addItemSection: some View {
        Section {
            TextField("Ingredient", text: $ingredientName)
                .focused($addFieldFocused)
                .submitLabel(.done)
                .onSubmit(addItem)
                .onAppear {
                    addFieldFocused = true
                }
            TextField("Amount (optional)", text: $amount)
                .submitLabel(.done)
                .onSubmit(addItem)
            HStack {
                Button("Add to List", action: addItem)
                    .disabled(ingredientName.trimmingCharacters(in: .whitespaces).isEmpty)
                Spacer()
                Button("Done") {
                    isAddingItem = false
                    addFieldFocused = false
                    ingredientName = ""
                    amount = ""
                }
                .foregroundColor(.secondary)
            }
        }
    }

    private var itemsList: some View {
        List {
            let checked = store.items.filter(\.isChecked)

            if isAddingItem {
                addItemSection
            }

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
        .listStyle(.plain)
    }

    private func addItem() {
        let name = ingredientName.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { return }

        let trimmedAmount = amount.trimmingCharacters(in: .whitespaces)
        store.addItem(
            name: name,
            amount: trimmedAmount.isEmpty ? nil : trimmedAmount
        )

        ingredientName = ""
        amount = ""
        addFieldFocused = true

        withAnimation {
            addedCount += 1
        }
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

    private var displayText: String {
        let name = item.item ?? ""
        if let amount = item.amount, !amount.isEmpty {
            return "\(name) · \(amount)"
        }
        return name
    }

    var body: some View {
        HStack(alignment: .center, spacing: 8) {
            Button {
                store.toggleChecked(item)
            } label: {
                HStack(alignment: .center, spacing: 8) {
                    Image(systemName: item.isChecked ? "checkmark.circle.fill" : "circle")
                        .font(.body)
                        .foregroundColor(item.isChecked ? .green : .secondary)

                    VStack(alignment: .leading, spacing: 1) {
                        Text(displayText)
                            .font(.body)
                            .strikethrough(item.isChecked)
                            .foregroundColor(item.isChecked ? .secondary : .primary)

                        if let recipeTitle = item.sourceRecipeTitle {
                            Text(recipeTitle)
                                .font(.caption)
                                .foregroundColor(.orange)
                        }
                    }

                    Spacer()
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            Menu {
                Button("Auto") {
                    store.updateCategoryOverride(item, categoryOverride: nil)
                }
                ForEach(store.categoryOrder, id: \.self) { category in
                    Button {
                        store.updateCategoryOverride(item, categoryOverride: category)
                    } label: {
                        HStack {
                            Text(category)
                            if item.categoryOverride == category {
                                Spacer()
                                Image(systemName: "checkmark")
                            }
                        }
                    }
                }
            } label: {
                Image(systemName: item.categoryOverride == nil ? "tag" : "tag.fill")
                    .foregroundColor(.secondary)
            }
            .accessibilityLabel("Category")
        }
        .listRowInsets(EdgeInsets(top: 6, leading: 16, bottom: 6, trailing: 16))
    }
}

#Preview {
    ShoppingListView()
}
