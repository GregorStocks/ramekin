const UNICODE_FRACTIONS: Record<string, string> = {
  "½": "1/2",
  "⅓": "1/3",
  "⅔": "2/3",
  "¼": "1/4",
  "¾": "3/4",
  "⅕": "1/5",
  "⅖": "2/5",
  "⅗": "3/5",
  "⅘": "4/5",
  "⅙": "1/6",
  "⅚": "5/6",
  "⅛": "1/8",
  "⅜": "3/8",
  "⅝": "5/8",
  "⅞": "7/8",
};

const UNIT_FRACTION_DENOMS = [2, 3, 4, 6, 8] as const;
const FLOAT_TOL = 1e-6;

function normalizeFractions(input: string): string {
  let out = "";
  for (const ch of input) {
    if (ch in UNICODE_FRACTIONS) {
      if (out.length > 0 && /[0-9]$/.test(out)) {
        out += " ";
      }
      out += UNICODE_FRACTIONS[ch];
    } else {
      out += ch;
    }
  }
  return out;
}

function parseAmount(raw: string): number | null {
  const s = normalizeFractions(raw).trim();
  if (s.length === 0) return null;

  const mixed = s.match(/^(\d+)\s+(\d+)\/(\d+)$/);
  if (mixed) {
    const whole = Number(mixed[1]);
    const num = Number(mixed[2]);
    const denom = Number(mixed[3]);
    if (denom === 0) return null;
    return whole + num / denom;
  }

  const frac = s.match(/^(\d+)\/(\d+)$/);
  if (frac) {
    const num = Number(frac[1]);
    const denom = Number(frac[2]);
    if (denom === 0) return null;
    return num / denom;
  }

  if (/^\d{1,3},\d{3}$/.test(s)) {
    return null;
  }

  if (/^\d+([\.,]\d+)?$/.test(s) || /^[\.,]\d+$/.test(s)) {
    return Number(s.replace(",", "."));
  }

  return null;
}

function formatScaled(value: number): string {
  const rounded = Math.round(value);
  if (Math.abs(value - rounded) < FLOAT_TOL) {
    return String(rounded);
  }

  for (const denom of UNIT_FRACTION_DENOMS) {
    if (Math.abs(value - 1 / denom) < FLOAT_TOL) {
      return `1/${denom}`;
    }
  }

  let out = value.toFixed(2);
  out = out.replace(/\.?0+$/, "");
  return out;
}

/**
 * Multiply an ingredient amount string by `factor` and re-format.
 *
 * Returns the original string unchanged when:
 *   - the amount cannot be parsed (free text, ranges, empty),
 *   - `factor` is not a positive finite number,
 *   - `factor === 1`.
 */
export function scaleAmount(
  amount: string | null | undefined,
  factor: number,
): string {
  if (amount == null || amount === "") return amount ?? "";
  if (!Number.isFinite(factor) || factor <= 0) return amount;
  if (factor === 1) return amount;

  const parsed = parseAmount(amount);
  if (parsed === null) return amount;

  return formatScaled(parsed * factor);
}
