import UIKit
import CoreImage.CIFilterBuiltins

extension RecipePDFRenderer {
    /// Tracks the current drawing position across pages of a UIGraphicsPDFRenderer
    /// rendering context. The caller drives layout top-down; this just remembers
    /// the cursor Y, opens new pages, and exposes the content rect.
    struct PageCursor {
        let margin: CGFloat
        let pageSize: CGSize
        let context: UIGraphicsPDFRendererContext
        var currentY: CGFloat

        init(margin: CGFloat, pageSize: CGSize, context: UIGraphicsPDFRendererContext) {
            self.margin = margin
            self.pageSize = pageSize
            self.context = context
            self.currentY = margin
        }

        var leftX: CGFloat { margin }
        var contentWidth: CGFloat { pageSize.width - margin * 2 }
        var bottomY: CGFloat { pageSize.height - margin }
        var remainingPageHeight: CGFloat { bottomY - currentY }

        mutating func beginPage() {
            context.beginPage()
            currentY = margin
        }

        mutating func advance(by delta: CGFloat) {
            currentY += delta
        }

        mutating func ensureSpace(for height: CGFloat) {
            if remainingPageHeight < height {
                beginPage()
            }
        }
    }

    static func bodyAttributes(italic: Bool = false) -> [NSAttributedString.Key: Any] {
        let font: UIFont = italic
            ? UIFont.italicSystemFont(ofSize: 12)
            : UIFont.systemFont(ofSize: 12)
        return [
            .font: font,
            .foregroundColor: UIColor.black
        ]
    }

    static func boundingHeight(for string: NSAttributedString, width: CGFloat) -> CGFloat {
        let rect = string.boundingRect(
            with: CGSize(width: width, height: .greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin],
            context: nil
        )
        return rect.height.rounded(.up)
    }

    static func nonEmpty(_ value: String?) -> String? {
        guard let value else { return nil }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    static func generateQRCodeImage(for text: String, size: CGFloat) -> UIImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(text.utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage else { return nil }
        let scaleX = size / output.extent.size.width
        let scaleY = size / output.extent.size.height
        let scaled = output.transformed(by: CGAffineTransform(scaleX: scaleX, y: scaleY))
        let context = CIContext()
        guard let cgImage = context.createCGImage(scaled, from: scaled.extent) else { return nil }
        return UIImage(cgImage: cgImage)
    }

    /// Draw a paragraph that may need to break across pages. The paragraph is
    /// chunked into lines using CoreText layout via NSAttributedString's
    /// boundingRect-driven slicing: we draw what fits on the page, then
    /// re-measure the remainder on the next page.
    static func drawParagraph(
        _ text: String,
        attributes: [NSAttributedString.Key: Any],
        cursor: inout PageCursor
    ) {
        var remaining = NSAttributedString(string: text, attributes: attributes)
        while remaining.length > 0 {
            let availableHeight = cursor.remainingPageHeight
            if availableHeight < 12 {
                cursor.beginPage()
                continue
            }
            let width = cursor.contentWidth
            let totalHeight = measureHeight(remaining, width: width)

            if totalHeight <= availableHeight {
                drawAttributedString(remaining, at: cursor, height: totalHeight)
                cursor.advance(by: totalHeight)
                return
            }

            let bestFit = longestPrefixFitting(
                remaining,
                width: width,
                availableHeight: availableHeight
            )
            if bestFit == 0 {
                cursor.beginPage()
                continue
            }

            let breakIndex = wordBoundary(in: remaining.string, upTo: bestFit) ?? bestFit
            let piece = remaining.attributedSubstring(from: NSRange(location: 0, length: breakIndex))
            let pieceHeight = measureHeight(piece, width: width)
            drawAttributedString(piece, at: cursor, height: pieceHeight)
            cursor.advance(by: pieceHeight)
            cursor.beginPage()
            remaining = remaining.attributedSubstring(
                from: NSRange(location: breakIndex, length: remaining.length - breakIndex)
            )
        }
    }

    static func measureHeight(_ string: NSAttributedString, width: CGFloat) -> CGFloat {
        let size = CGSize(width: width, height: .greatestFiniteMagnitude)
        return string.boundingRect(
            with: size,
            options: [.usesLineFragmentOrigin],
            context: nil
        ).height.rounded(.up)
    }

    static func drawAttributedString(
        _ string: NSAttributedString,
        at cursor: PageCursor,
        height: CGFloat
    ) {
        string.draw(
            with: CGRect(
                x: cursor.leftX,
                y: cursor.currentY,
                width: cursor.contentWidth,
                height: height
            ),
            options: [.usesLineFragmentOrigin],
            context: nil
        )
    }

    /// Binary search for the longest prefix of `string` whose height at `width`
    /// is no greater than `availableHeight`.
    static func longestPrefixFitting(
        _ string: NSAttributedString,
        width: CGFloat,
        availableHeight: CGFloat
    ) -> Int {
        var low = 1
        var high = string.length
        var best = 0
        while low <= high {
            let mid = (low + high) / 2
            let prefix = string.attributedSubstring(from: NSRange(location: 0, length: mid))
            if measureHeight(prefix, width: width) <= availableHeight {
                best = mid
                low = mid + 1
            } else {
                high = mid - 1
            }
        }
        return best
    }

    /// Scan backwards from `length` looking for whitespace to break on. Returns
    /// the index after the whitespace, or nil if no whitespace is found.
    static func wordBoundary(in text: String, upTo length: Int) -> Int? {
        let nsString = text as NSString
        var index = length
        let whitespace = CharacterSet.whitespacesAndNewlines
        while index > 0 {
            let unit = nsString.character(at: index - 1)
            if let scalar = Unicode.Scalar(unit), whitespace.contains(scalar) {
                return index
            }
            index -= 1
        }
        return nil
    }
}
