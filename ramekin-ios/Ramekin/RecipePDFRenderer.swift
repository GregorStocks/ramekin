import UIKit
import CoreImage.CIFilterBuiltins

/// Render a recipe to a single PDF file in the temp directory using
/// `UIGraphicsPDFRenderer`. The layout is intentionally simple: a header
/// (title + optional source link / QR), an optional cover photo, an
/// ingredients section, then instructions. Long content flows across
/// pages by drawing into successive UIGraphicsPDFRenderer pages.
enum RecipePDFRenderer {
    /// US Letter at 72dpi (PDF user space defaults to 1pt = 1/72 inch).
    private static let pageSize = CGSize(width: 612, height: 792)
    private static let margin: CGFloat = 48
    private static let qrCodeSize: CGFloat = 96
    private static let coverPhotoMaxHeight: CGFloat = 280

    struct Result {
        let url: URL
        let data: Data
    }

    static func render(
        recipe: RecipeResponse,
        coverPhoto: UIImage?,
        filename: String? = nil,
        fileManager: FileManager = .default
    ) throws -> Result {
        let renderer = UIGraphicsPDFRenderer(
            bounds: CGRect(origin: .zero, size: pageSize),
            format: UIGraphicsPDFRendererFormat()
        )

        let data = renderer.pdfData { context in
            var cursor = PageCursor(margin: margin, pageSize: pageSize, context: context)
            cursor.beginPage()

            drawHeader(recipe: recipe, cursor: &cursor)
            cursor.advance(by: 12)

            if let photo = coverPhoto {
                drawCoverPhoto(photo, cursor: &cursor)
                cursor.advance(by: 16)
            }

            if let description = nonEmpty(recipe.description) {
                drawParagraph(description, attributes: bodyAttributes(italic: true), cursor: &cursor)
                cursor.advance(by: 12)
            }

            drawMetadata(recipe: recipe, cursor: &cursor)

            if !recipe.ingredients.isEmpty {
                cursor.advance(by: 16)
                drawIngredients(recipe.ingredients, cursor: &cursor)
            }

            cursor.advance(by: 16)
            drawInstructions(recipe.instructions, cursor: &cursor)

            if let notes = nonEmpty(recipe.notes) {
                cursor.advance(by: 16)
                drawSectionHeader("Notes", cursor: &cursor)
                drawParagraph(notes, attributes: bodyAttributes(), cursor: &cursor)
            }
        }

        let safeName = filename ?? defaultFilename(for: recipe.title)
        let directory = fileManager.temporaryDirectory.appendingPathComponent(
            "ramekin-exports/\(UUID().uuidString)",
            isDirectory: true
        )
        try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
        let url = directory.appendingPathComponent(safeName)
        try data.write(to: url, options: .atomic)
        return Result(url: url, data: data)
    }

    /// Build a sensible filename from a recipe title. Exposed for tests.
    static func defaultFilename(for title: String) -> String {
        let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines)
        let slug = trimmed.unicodeScalars
            .map { scalar -> Character in
                if CharacterSet.alphanumerics.contains(scalar) {
                    return Character(scalar)
                }
                return "-"
            }
        let collapsed = String(slug)
            .split(separator: "-", omittingEmptySubsequences: true)
            .joined(separator: "-")
        let base = collapsed.isEmpty ? "recipe" : String(collapsed.prefix(64))
        return "\(base).pdf"
    }

    // MARK: - Sections

    private static func drawHeader(recipe: RecipeResponse, cursor: inout PageCursor) {
        let availableWidth = cursor.contentWidth
        let qrURL = recipe.sourceUrl.flatMap(URL.init(string:))
        let hasQR = qrURL != nil
        let titleWidth = hasQR ? availableWidth - qrCodeSize - 16 : availableWidth

        let titleAttrs: [NSAttributedString.Key: Any] = [
            .font: UIFont.boldSystemFont(ofSize: 24),
            .foregroundColor: UIColor.black
        ]
        let titleString = NSAttributedString(string: recipe.title, attributes: titleAttrs)
        let titleHeight = boundingHeight(for: titleString, width: titleWidth)

        let qrHeight: CGFloat = hasQR ? qrCodeSize + 14 : 0
        let blockHeight = max(titleHeight, qrHeight)
        cursor.ensureSpace(for: blockHeight)

        let topY = cursor.currentY
        titleString.draw(in: CGRect(
            x: cursor.leftX,
            y: topY,
            width: titleWidth,
            height: titleHeight
        ))

        if let qrURL {
            let qrX = cursor.leftX + availableWidth - qrCodeSize
            if let qrImage = generateQRCodeImage(for: qrURL.absoluteString, size: qrCodeSize) {
                qrImage.draw(in: CGRect(x: qrX, y: topY, width: qrCodeSize, height: qrCodeSize))
            }
            let label = recipe.sourceName?.trimmingCharacters(in: .whitespacesAndNewlines)
            let labelText = (label?.isEmpty == false ? label! : qrURL.host) ?? qrURL.absoluteString
            let linkAttrs: [NSAttributedString.Key: Any] = [
                .font: UIFont.systemFont(ofSize: 9),
                .foregroundColor: UIColor.darkGray
            ]
            let linkString = NSAttributedString(string: labelText, attributes: linkAttrs)
            linkString.draw(in: CGRect(
                x: qrX,
                y: topY + qrCodeSize + 2,
                width: qrCodeSize,
                height: 12
            ))
        }

        cursor.advance(by: blockHeight)
    }

    private static func drawCoverPhoto(_ photo: UIImage, cursor: inout PageCursor) {
        let availableWidth = cursor.contentWidth
        let aspect = photo.size.height == 0 ? 1 : photo.size.width / photo.size.height
        let drawWidth = availableWidth
        var drawHeight = drawWidth / max(aspect, 0.01)
        if drawHeight > coverPhotoMaxHeight {
            drawHeight = coverPhotoMaxHeight
        }
        cursor.ensureSpace(for: drawHeight)
        photo.draw(in: CGRect(
            x: cursor.leftX,
            y: cursor.currentY,
            width: drawWidth,
            height: drawHeight
        ))
        cursor.advance(by: drawHeight)
    }

    private static func drawMetadata(recipe: RecipeResponse, cursor: inout PageCursor) {
        var pieces: [String] = []
        if let value = nonEmpty(recipe.prepTime) { pieces.append("Prep: \(value)") }
        if let value = nonEmpty(recipe.cookTime) { pieces.append("Cook: \(value)") }
        if let value = nonEmpty(recipe.totalTime) { pieces.append("Total: \(value)") }
        if let value = nonEmpty(recipe.servings) { pieces.append("Serves: \(value)") }
        if let value = nonEmpty(recipe.difficulty) { pieces.append("Difficulty: \(value)") }
        if let rating = recipe.rating, (1...5).contains(rating) {
            pieces.append("Rating: \(String(repeating: "★", count: rating))")
        }
        guard !pieces.isEmpty else { return }
        let line = pieces.joined(separator: "  •  ")
        drawParagraph(
            line,
            attributes: [
                .font: UIFont.systemFont(ofSize: 11),
                .foregroundColor: UIColor.darkGray
            ],
            cursor: &cursor
        )
    }

    private static func drawIngredients(_ ingredients: [Ingredient], cursor: inout PageCursor) {
        drawSectionHeader("Ingredients", cursor: &cursor)
        var currentSection: String?
        for ingredient in ingredients {
            let section = ingredient.section?.trimmingCharacters(in: .whitespacesAndNewlines)
            if let section, !section.isEmpty, section != currentSection {
                currentSection = section
                cursor.advance(by: 4)
                drawParagraph(
                    section,
                    attributes: [
                        .font: UIFont.boldSystemFont(ofSize: 12),
                        .foregroundColor: UIColor.black
                    ],
                    cursor: &cursor
                )
            } else if section == nil || section?.isEmpty == true {
                currentSection = nil
            }
            let text = ingredient.formatted(
                scale: 1,
                includeAlternatives: true,
                includeNote: true
            )
            drawParagraph(
                "• \(text)",
                attributes: bodyAttributes(),
                cursor: &cursor
            )
        }
    }

    private static func drawInstructions(_ instructions: String, cursor: inout PageCursor) {
        drawSectionHeader("Instructions", cursor: &cursor)
        let trimmed = instructions.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        drawParagraph(trimmed, attributes: bodyAttributes(), cursor: &cursor)
    }

    // MARK: - Drawing primitives

    private static func drawSectionHeader(_ text: String, cursor: inout PageCursor) {
        drawParagraph(
            text,
            attributes: [
                .font: UIFont.boldSystemFont(ofSize: 16),
                .foregroundColor: UIColor.black
            ],
            cursor: &cursor
        )
        cursor.advance(by: 4)
    }

}
