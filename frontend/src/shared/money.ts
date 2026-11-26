export function parseMinorUnits(raw: string, allowNegative = false): number | null {
  let normalized = raw.trim();
  let sign = 1;
  if (normalized.startsWith("-")) {
    if (!allowNegative) return null;
    sign = -1;
    normalized = normalized.slice(1);
  }

  // Accept ungrouped digits, Western grouping (1,000,000), or Indian grouping
  // (1,23,456). Validate grouping before removing separators.
  const amountPattern = /^(?:\d+|\d{1,3}(?:,\d{3})+|\d{1,2}(?:,\d{2})+,\d{3})(?:\.\d{1,2})?$/;
  if (!amountPattern.test(normalized)) return null;

  const [integerText, fraction = ""] = normalized.split(".");
  const major = Number(integerText.replace(/,/g, ""));
  const minor = Number(fraction.padEnd(2, "0"));
  if (!Number.isSafeInteger(major) || !Number.isSafeInteger(minor)) return null;
  const total = major * 100 + minor;
  if (!Number.isSafeInteger(total)) return null;
  return sign * total;
}

export function formatMinor(minor: number, currency = "INR"): string {
  return new Intl.NumberFormat("en-IN", {
    style: "currency",
    currency,
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(minor / 100);
}
