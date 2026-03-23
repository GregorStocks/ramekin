import SwiftUI

struct IngredientRowView: View {
    @Binding var ingredient: EditableIngredient
    var onDelete: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            primaryMeasurementRow
            noteField
            alternativeMeasurements
            actionButtons
        }
        .padding(.vertical, 4)
    }

    // MARK: - Subviews

    private var primaryMeasurementRow: some View {
        HStack(spacing: 8) {
            TextField("Amt", text: primaryAmountBinding)
                .frame(width: 50)
                .keyboardType(.decimalPad)
            TextField("Unit", text: primaryUnitBinding)
                .frame(width: 60)
            TextField("Ingredient", text: $ingredient.item)
        }
        .font(.body)
    }

    @ViewBuilder
    private var noteField: some View {
        if !ingredient.note.isEmpty || ingredient.item.isEmpty {
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
                    .frame(width: 30)
                TextField("Amt", text: measurementAmountBinding(mIdx))
                    .frame(width: 50)
                    .keyboardType(.decimalPad)
                TextField("Unit", text: measurementUnitBinding(mIdx))
                    .frame(width: 60)
                Spacer()
                Button {
                    ingredient.measurements.remove(at: mIdx)
                } label: {
                    Image(systemName: "minus.circle")
                        .foregroundColor(.red)
                        .font(.caption)
                }
                .buttonStyle(.plain)
            }
            .font(.caption)
        }
    }

    private var actionButtons: some View {
        HStack(spacing: 16) {
            Button {
                ingredient.measurements.append(EditableMeasurement())
            } label: {
                Label("Alt measurement", systemImage: "plus.circle")
                    .font(.caption2)
                    .foregroundColor(.orange)
            }
            .buttonStyle(.plain)

            if ingredient.note.isEmpty && !ingredient.item.isEmpty {
                Button {
                    ingredient.note = " "
                } label: {
                    Label("Note", systemImage: "note.text")
                        .font(.caption2)
                        .foregroundColor(.orange)
                }
                .buttonStyle(.plain)
            }

            Spacer()

            Button(action: onDelete) {
                Image(systemName: "trash")
                    .font(.caption)
                    .foregroundColor(.red)
            }
            .buttonStyle(.plain)
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
