import SwiftUI

struct EditMealPlanSheet: View {
    @Environment(\.dismiss) private var dismiss

    let meal: MealPlanItem
    let onSave: (Date, MealType, String) async throws -> Void

    @State private var selectedDate: Date
    @State private var selectedMealType: MealType
    @State private var notes: String
    @State private var isSaving = false
    @State private var error: String?

    init(
        meal: MealPlanItem,
        onSave: @escaping (Date, MealType, String) async throws -> Void
    ) {
        self.meal = meal
        self.onSave = onSave
        _selectedDate = State(initialValue: MealPlanDateSupport.localDate(fromAPIDate: meal.mealDate))
        _selectedMealType = State(initialValue: meal.mealType)
        _notes = State(initialValue: meal.notes ?? "")
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Text(meal.recipeTitle)
                        .font(.headline)

                    DatePicker("Date", selection: $selectedDate, displayedComponents: .date)

                    Picker("Meal", selection: $selectedMealType) {
                        ForEach(MealType.displayOrder, id: \.self) { mealType in
                            Text(mealType.displayLabel).tag(mealType)
                        }
                    }
                }

                Section("Notes") {
                    TextField("Optional notes", text: $notes, axis: .vertical)
                        .lineLimit(3, reservesSpace: true)
                }

                if let error {
                    Section {
                        Text(error)
                            .foregroundColor(.red)
                    }
                }
            }
            .navigationTitle("Edit Meal")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                    .disabled(isSaving)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        Task { await save() }
                    }
                    .disabled(isSaving)
                }
            }
        }
    }

    private func save() async {
        isSaving = true
        error = nil

        do {
            try await onSave(selectedDate, selectedMealType, notes)
            await MainActor.run {
                isSaving = false
                dismiss()
            }
        } catch is CancellationError {
            await MainActor.run {
                isSaving = false
            }
        } catch {
            let message = APIErrorFormatter.code(from: error) == .conflict
                ? "This recipe is already planned for that meal."
                : APIErrorFormatter.userMessage(from: error, fallback: "Failed to update meal plan")
            await MainActor.run {
                self.error = message
                isSaving = false
            }
        }
    }
}
