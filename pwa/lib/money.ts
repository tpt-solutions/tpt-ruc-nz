/** Micro-dollars per dollar. Money is held in micro-dollars across the stack. */
export const MICROS_PER_DOLLAR = 1_000_000;

/**
 * Formats a micro-dollar amount as NZ dollars, for example `1_234_500` as `$1.23`.
 * Rounds half away from zero to the nearest cent.
 */
export function formatMicros(micros: number): string {
    if (!Number.isInteger(micros)) {
        throw new RangeError("micro-dollar amounts must be whole numbers");
    }
    const cents = Math.round(micros / (MICROS_PER_DOLLAR / 100));
    const sign = cents < 0 ? "-" : "";
    const absolute = Math.abs(cents);
    const dollars = Math.floor(absolute / 100);
    const remainder = String(absolute % 100).padStart(2, "0");
    return `${sign}$${dollars.toLocaleString("en-NZ")}.${remainder}`;
}
