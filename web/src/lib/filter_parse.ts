import type { CountryCode, PopularityTier } from '$lib/types';

const COUNTRY_CODES: CountryCode[] = ['US', 'FR', 'CL'];
const POPULARITY_TIERS: PopularityTier[] = ['extra_rare', 'rare', 'common', 'top'];
const GENDERS = ['any', 'unisex', 'mostly_male', 'mostly_female', 'male', 'female'] as const;

function isCountryCode(v: string): v is CountryCode {
  return (COUNTRY_CODES as readonly string[]).includes(v);
}

function isPopularityTier(v: string): v is PopularityTier {
  return (POPULARITY_TIERS as readonly string[]).includes(v);
}

function isGender(v: string): v is (typeof GENDERS)[number] {
  return (GENDERS as readonly string[]).includes(v);
}

export function parseCountriesCsv(input: string | null | undefined): CountryCode[] {
  const items = (input ?? '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)
    .filter(isCountryCode);
  return items.length ? items : ['US', 'FR', 'CL'];
}

export function parsePopularityCsv(input: string | null | undefined): PopularityTier[] {
  const items = (input ?? '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)
    .filter(isPopularityTier);
  return items.length ? items : ['rare', 'common', 'top'];
}

export function parseGender(input: string | null | undefined): (typeof GENDERS)[number] {
  const v = (input ?? '').trim();
  return isGender(v) ? v : 'any';
}

