/**
 * Utility functions for working with Candid types in the frontend
 */

/**
 * Unwraps a Candid optional type (which comes as an array: [] for None, [value] for Some)
 * @param {Array} candidOpt - The Candid optional value
 * @returns {*} The unwrapped value or null
 */
export function unwrapOpt(candidOpt) {
    return candidOpt && candidOpt.length > 0 ? candidOpt[0] : null;
}

/**
 * Unwraps a Candid optional number and converts BigInt to Number
 * @param {Array} candidOpt - The Candid optional value
 * @returns {number|null} The unwrapped number or null
 */
export function unwrapOptNum(candidOpt) {
    if (!candidOpt || candidOpt.length === 0) return null;
    const val = candidOpt[0];
    return typeof val === 'bigint' ? Number(val) : val;
}

/**
 * Converts a Principal to string, handling both Principal objects and strings
 * @param {Principal|string} principal - The principal to convert
 * @returns {string} The principal as a string
 */
export function principalToString(principal) {
    if (!principal) return '';
    if (typeof principal === 'string') return principal;
    if (principal.toText) return principal.toText();
    return principal.toString();
}

/**
 * Formats chip amounts for display (e.g., 1000 -> "1K", 1000000 -> "1M")
 * @param {number|bigint} amount - The chip amount
 * @returns {string} Formatted string
 */
export function formatChips(amount) {
    const num = Number(amount);
    if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`;
    if (num >= 1000) return `${(num / 1000).toFixed(1)}K`;
    return num.toLocaleString();
}

/**
 * Gets the phase name from a Candid variant
 * @param {Object} phase - The phase variant
 * @returns {string} The phase name
 */
export function getPhaseName(phase) {
    if (!phase) return 'Unknown';
    return Object.keys(phase)[0];
}
