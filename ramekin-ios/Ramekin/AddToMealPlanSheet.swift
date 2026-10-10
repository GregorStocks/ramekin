import SwiftUI

struct AddToMealPlanSheet: View {
    let recipe: RecipeResponse
    @Binding var isPresented: Bool

    @State private var selectedDate: Date = Date()
    @State private var selectedMealType: MealType = .dinner
    @State private var isAdding = false
    @State private var showingConfirmation = false
    @State private var error: String?
    @State private var submitTask: Task<Void, Never>?
    @State private var dismissTask: Task<Void, Never>?

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
                        cancelPendingWork()
                        isPresented = false
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") {
                        // Mark the submission active before spawning it so a
                        // second tap can't start an untracked submission.
                        guard !isAdding else { return }
                        isAdding = true
                        submitTask = Task { await addToMealPlan() }
                    }
                    // Stays disabled during the success overlay so a second
                    // tap can't schedule the recipe twice.
                    .disabled(isAdding || showingConfirmation)
                }
            }
            .onDisappear {
                // A swipe-down dismissal must not leave a pending submission
                // or close that would dismiss the next presentation.
                cancelPendingWork()
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
                .scaledIconFont(size: 50)
                .accessibilityHidden(true)
                .foregroundColor(.green)
            Text("Added to meal plan")
                .font(.headline)
        }
        .padding(30)
        .background(.regularMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 16))
    }

    @MainActor
    private func cancelPendingWork() {
        submitTask?.cancel()
        submitTask = nil
        SheetAutoDismissSupport.cancel(&dismissTask)
    }

    @MainActor
    private func addToMealPlan() async {
        error = nil

        do {
            _ = try await RamekinAPI.shared.createMealPlan(
                recipeId: recipe.id,
                mealDate: selectedDate,
                mealType: selectedMealType.rawValue
            )
            // The sheet was dismissed while the request was in flight.
            guard !Task.isCancelled else { return }
            isAdding = false
            showingConfirmation = true
            UIAccessibility.post(notification: .announcement, argument: "Added to meal plan")
            SheetAutoDismissSupport.schedule(&dismissTask) {
                isPresented = false
            }
        } catch is CancellationError {
            // ignored
        } catch {
            guard !Task.isCancelled else { return }
            self.error = error.localizedDescription
            isAdding = false
        }
    }
}
