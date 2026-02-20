import SwiftUI

struct AddIngredientSheet: View {
    @Binding var isPresented: Bool
    @State private var ingredientName = ""
    @State private var amount = ""
    @State private var addedCount = 0
    @FocusState private var nameFieldFocused: Bool

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Ingredient", text: $ingredientName)
                        .focused($nameFieldFocused)
                        .submitLabel(.done)
                        .onSubmit(addItem)
                    TextField("Amount (optional)", text: $amount)
                        .submitLabel(.done)
                        .onSubmit(addItem)
                }

                Section {
                    Button(action: addItem) {
                        Text("Add to List")
                            .frame(maxWidth: .infinity)
                    }
                    .disabled(ingredientName.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
            .navigationTitle("Add Ingredient")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") {
                        isPresented = false
                    }
                }
            }
            .onAppear {
                nameFieldFocused = true
            }
            .overlay(alignment: .bottom) {
                if addedCount > 0 {
                    addedBanner
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                }
            }
        }
    }

    private var addedBanner: some View {
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
    }

    private func addItem() {
        let name = ingredientName.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { return }

        let trimmedAmount = amount.trimmingCharacters(in: .whitespaces)
        ShoppingListStore.shared.addItem(
            name: name,
            amount: trimmedAmount.isEmpty ? nil : trimmedAmount
        )

        ingredientName = ""
        amount = ""
        nameFieldFocused = true

        withAnimation {
            addedCount += 1
        }
    }
}
