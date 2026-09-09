import SwiftUI

struct IngredientRowView: View {
    @Binding var ingredient: EditableIngredient
    var onDelete: () -> Void
    @State private var isNoteVisible: Bool

    init(ingredient: Binding<EditableIngredient>, onDelete: @escaping () -> Void) {
        _ingredient = ingredient
        self.onDelete = onDelete
        _isNoteVisible = State(
            initialValue: IngredientRowViewSupport.initialNoteVisibility(
                item: ingredient.wrappedValue.item,
                note: ingredient.wrappedValue.note
            )
        )
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            primaryMeasurementRow
            noteField
            alternativeMeasurements
            actionButtons
        }
        .padding(.vertical, 4)
        .onChange(of: ingredient.id) { _ in
            isNoteVisible = IngredientRowViewSupport.initialNoteVisibility(
                item: ingredient.item,
                note: ingredient.note
            )
        }
    }

    // MARK: - Subviews

    private var primaryMeasurementRow: some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField("Ingredient", text: $ingredient.item)
                .accessibilityLabel("Ingredient")
            HStack(alignment: .top, spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Amount").font(.caption).foregroundColor(.secondary)
                    TextField("Amount", text: primaryAmountBinding)
                        .keyboardType(.decimalPad)
                        .accessibilityLabel("Amount")
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                VStack(alignment: .leading, spacing: 4) {
                    Text("Unit").font(.caption).foregroundColor(.secondary)
                    TextField("Unit", text: primaryUnitBinding)
                        .accessibilityLabel("Unit")
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .font(.body)
    }

    @ViewBuilder
    private var noteField: some View {
        if IngredientRowViewSupport.shouldShowNoteField(
            item: ingredient.item,
            note: ingredient.note,
            isNoteVisible: isNoteVisible
        ) {
            TextField("Note (e.g., chopped)", text: $ingredient.note)
                .font(.caption)
                .foregroundColor(.secondary)
        }
    }

    @ViewBuilder
    private var alternativeMeasurements: some View {
        let altIndices = ingredient.measurements.count > 1
            ? Array(1..<ingredient.measurements.count) : []
        ForEach(altIndices, id: \.self) { mIdx in
            HStack(spacing: 8) {
                Text("Alt:")
                    .font(.caption)
                    .foregroundColor(.secondary)
                TextField("Amount", text: measurementAmountBinding(mIdx))
                    .frame(maxWidth: .infinity)
                    .keyboardType(.decimalPad)
                    .accessibilityLabel("Alternative amount")
                TextField("Unit", text: measurementUnitBinding(mIdx))
                    .frame(maxWidth: .infinity)
                    .accessibilityLabel("Alternative unit")
                Button {
                    ingredient.measurements.remove(at: mIdx)
                } label: {
                    Image(systemName: "minus.circle")
                        .foregroundColor(.red)
                        .font(.body)
                        .frame(minWidth: 44, minHeight: 44)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Remove alternative measurement")
            }
            .font(.body)
        }
    }

    private var actionButtons: some View {
        HStack(spacing: 8) {
            Button {
                ingredient.measurements.append(EditableMeasurement())
            } label: {
                Label("Alt measurement", systemImage: "plus.circle")
                    .font(.subheadline)
                    .foregroundColor(.primary)
                    .frame(minHeight: 44)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            if IngredientRowViewSupport.shouldShowAddNoteButton(
                item: ingredient.item,
                note: ingredient.note,
                isNoteVisible: isNoteVisible
            ) {
                Button {
                    isNoteVisible = true
                } label: {
                    Label("Note", systemImage: "note.text")
                        .font(.subheadline)
                        .foregroundColor(.primary)
                        .frame(minHeight: 44)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }

            Spacer()

            Button(action: onDelete) {
                Image(systemName: "trash")
                    .font(.body)
                    .foregroundColor(.red)
                    .frame(minWidth: 44, minHeight: 44)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Remove ingredient")
        }
    }

    // MARK: - Measurement Bindings

    private var primaryAmountBinding: Binding<String> {
        Binding(
            get: { ingredient.measurements.first?.amount ?? "" },
            set: { newValue in
                ensurePrimaryMeasurement()
                ingredient.measurements[0].amount = newValue
            }
        )
    }

    private var primaryUnitBinding: Binding<String> {
        Binding(
            get: { ingredient.measurements.first?.unit ?? "" },
            set: { newValue in
                ensurePrimaryMeasurement()
                ingredient.measurements[0].unit = newValue
            }
        )
    }

    private func measurementAmountBinding(_ index: Int) -> Binding<String> {
        Binding(
            get: { ingredient.measurements[index].amount },
            set: { ingredient.measurements[index].amount = $0 }
        )
    }

    private func measurementUnitBinding(_ index: Int) -> Binding<String> {
        Binding(
            get: { ingredient.measurements[index].unit },
            set: { ingredient.measurements[index].unit = $0 }
        )
    }

    private func ensurePrimaryMeasurement() {
        if ingredient.measurements.isEmpty {
            ingredient.measurements = [EditableMeasurement()]
        }
    }
}

enum IngredientRowViewSupport {
    static func initialNoteVisibility(item: String, note: String) -> Bool {
        shouldShowNoteField(item: item, note: note, isNoteVisible: false)
    }

    static func shouldShowNoteField(item: String, note: String, isNoteVisible: Bool) -> Bool {
        isNoteVisible || !note.isEmpty || item.isEmpty
    }

    static func shouldShowAddNoteButton(item: String, note: String, isNoteVisible: Bool) -> Bool {
        !item.isEmpty && note.isEmpty && !isNoteVisible
    }
}
