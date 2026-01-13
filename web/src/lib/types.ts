export type CountryCode = 'US' | 'FR' | 'CL';

export type CardKind = 'group' | 'concept';

export type GenderLabel =
  | 'unisex'
  | 'mostly_male'
  | 'mostly_female'
  | 'male'
  | 'female'
  | 'unknown';

export type PopularityTier = 'extra_rare' | 'rare' | 'common' | 'top';

export type SwipeAction = 'nope' | 'like' | 'superlike';

export type PerCountryStats = {
  country: CountryCode;
  ppm: number;
  top100_any: boolean;
  gender: GenderLabel;
};

export type Card = {
  card_key: string;
  card_kind: CardKind;
  display_name: string;
  spellings_preview: string[];
  score?: number;
  per_country: PerCountryStats[];
  sex_split?: Partial<
    Record<CountryCode, { male: number; female: number; male_pct: number; female_pct: number }>
  >;
  all_spellings?: string[];
  within_concept_variants?: string[];
  last5_years?: Partial<Record<CountryCode, number[]>>;
  last5_year_ranks?: Partial<
    Record<CountryCode, Array<{ year: number; rank: number; total?: number }>>
  >;
};

export type User = {
  id: number;
  username: string;
  is_admin: boolean;
};
