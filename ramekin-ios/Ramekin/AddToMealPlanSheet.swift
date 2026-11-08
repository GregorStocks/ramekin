import SwiftUI

struct AddToMealPlanSheet: View {
    let recipe: RecipeResponse
    @Binding var isPresented: Bool

    @State private var selectedDate: Date = Date()
    @State private var selectedMealType: MealType = .dinner
    @StateObject private var flow = AddSheetFlow()

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    DatePicker("Date", selection: $selectedDate, displayedComponents: .date)

                    Picker("Meal", selection: $selectedMealType) {
                        ForEach(MealType.displayOrder, id: \.self) { mealType in
                            Text(mealType.displayLabel).tag(mealType)
                        }
                    }
                }
                // Frozen under the confirmation overlay.
                .disabled(flow.isConfirmed)

                if let error = flow.error {
                    Section {
                        Text(error)
                            .foregroundColor(.red)
                    }
                }
            }
            .navigationTitle("Add to Meal Plan")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        flow.cancel()
                        isPresented = false
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") {
                        addToMealPlan()
                    }
                    // Stays disabled during the success overlay so a second
                    // tap can't schedule the recipe twice.
                    .disabled(flow.isConfirmed)
                }
            }
            .onDisappear {
                // A swipe-down dismissal must not leave a pending close that
                // would dismiss the next presentation.
                flow.cancel()
            }
            .overlay {
                if let confirmation = flow.confirmation {
                    AddSheetConfirmation(message: confirmation)
                }
            }
        }
    }

    private func addToMealPlan() {
        flow.submit(
            confirmation: "Added to meal plan",
            dismiss: { isPresented = false },
            add: {
                // Saved offline; the store syncs it.
                try MealPlanStore.shared.add(
                    recipeId: recipe.id,
                    recipeTitle: recipe.title,
                    thumbnailPhotoId: recipe.photoIds.first,
                    date: selectedDate,
                    mealType: selectedMealType
                )
            }
        )
    }
}

/// The check-mark overlay an add sheet shows before dismissing itself.
struct AddSheetConfirmation: View {
    let message: String

    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "checkmark.circle.fill")
                .scaledIconFont(size: 50)
                .accessibilityHidden(true)
                .foregroundColor(.green)
            Text(message)
                .font(.headline)
        }
        .padding(30)
        .background(.regularMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 16))
    }
}
