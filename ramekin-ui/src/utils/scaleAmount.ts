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

  const mixed = s.match(/^(\d+)(?:\s+|-)(\d+)\/(\d+)$/);
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

  if (/^[1-9]\d{0,2},\d{3}$/.test(s)) {
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

function scaleNumeric(raw: string, factor: number): string | null {
  const format = (value: number) => {
    const scaled = value * factor;
    return Number.isFinite(scaled) && Math.abs(scaled) <= 1e15
      ? formatScaled(scaled)
      : null;
  };
  const parsed = parseAmount(raw);
  if (parsed !== null) return format(parsed);
  for (const match of raw.matchAll(/\s+(?:to|or)\s+|\s*[-–—]\s*/gi)) {
    const left = parseAmount(raw.slice(0, match.index));
    const right = parseAmount(raw.slice(match.index! + match[0].length));
    if (left === null || right === null || left > right) continue;
    const low = format(left);
    const high = format(right);
    if (low !== null && high !== null) return `${low}${match[0]}${high}`;
  }
  return null;
}

function scaleTerm(raw: string, factor: number): string | null {
  const numeric = scaleNumeric(raw, factor);
  if (numeric !== null) return numeric;
  const unit = raw.match(
    /^(.+?)(\s+(?:g|grams?|kg|kilograms?|mg|milligrams?|oz|ounces?|lbs?|pounds?|cups?|tbsp|tablespoons?|tsp|teaspoons?|fl oz|fluid ounces?|pints?|quarts?|gallons?|ml|milliliters?|l|liters?|litres?|servings?))$/i,
  );
  if (!unit) return null;
  const scaled = scaleNumeric(unit[1], factor);
  return scaled === null ? null : `${scaled}${unit[2]}`;
}

/**
 * Multiply an ingredient amount string by `factor` and re-format.
 *
 * Returns the original string unchanged when:
 *   - the amount cannot be parsed (free text or invalid quantities),
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

  const serves = amount.match(/^(serves\s+)(.+)$/i);
  if (serves) {
    const scaled = scaleNumeric(serves[2], factor);
    return scaled === null ? amount : `${serves[1]}${scaled}`;
  }
  const parts = amount.split(/(\s+(?:plus|\+)\s+)/i);
  const scaled = parts.map((part, index) =>
    index % 2 === 1 ? part : scaleTerm(part, factor),
  );
  return scaled.some((part) => part === null) ? amount : scaled.join("");
}
