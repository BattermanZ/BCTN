import { describe, expect, it } from 'vitest';

import { parseCountriesCsv, parseGender, parsePopularityCsv } from '$lib/filter_parse';

describe('filter_parse', () => {
  it('parses valid countries and falls back when empty/invalid', () => {
    expect(parseCountriesCsv('US, FR, XX,CL')).toEqual(['US', 'FR', 'CL']);
    expect(parseCountriesCsv(' ,xx')).toEqual(['US', 'FR', 'CL']);
  });

  it('parses valid popularity tiers and falls back when empty/invalid', () => {
    expect(parsePopularityCsv('top, rare, nope,common')).toEqual(['top', 'rare', 'common']);
    expect(parsePopularityCsv('')).toEqual(['rare', 'common', 'top']);
  });

  it('parses known gender and falls back to any', () => {
    expect(parseGender('mostly_male')).toBe('mostly_male');
    expect(parseGender('unknown-value')).toBe('any');
    expect(parseGender(null)).toBe('any');
  });
});
