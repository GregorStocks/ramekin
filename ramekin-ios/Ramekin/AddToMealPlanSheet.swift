import SwiftUI

struct AddToMealPlanSheet: View {
    let recipe: RecipeResponse
    @Binding var isPresented: Bool

    @State private var selectedDate: Date = Date()
    @State private var selectedMealType: MealType = .dinner
    @State private var isAdding = false
    @State private var showingConfirmation = false
    @State private var error: String?

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

                if let error = error {
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
                        isPresented = false
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") {
                        Task { await addToMealPlan() }
                    }
                    .disabled(isAdding)
                }
            }
            .overlay {
                if showingConfirmation {
                    confirmationOverlay
                }
            }
        }
    }

    private var confirmationOverlay: some View {
        VStack(spacing: 12) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 50))
                .foregroundColor(.green)
            Text("Added to meal plan")
                .font(.headline)
        }
        .padding(30)
        .background(.regularMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 16))
    }

    private func addToMealPlan() async {
        isAdding = true
        error = nil

        do {
            _ = try await RamekinAPI.shared.createMealPlan(
                recipeId: recipe.id,
                mealDate: selectedDate,
                mealType: selectedMealType.rawValue
            )
            await MainActor.run {
                showingConfirmation = true
            }
            try? await Task.sleep(nanoseconds: 1_000_000_000)
            await MainActor.run {
                isPresented = false
            }
        } catch is CancellationError {
            // ignored
        } catch {
            await MainActor.run {
                self.error = error.localizedDescription
                isAdding = false
            }
        }
    }
}
