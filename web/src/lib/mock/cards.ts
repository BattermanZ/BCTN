import type { Card, CountryCode, GenderLabel } from '$lib/types';

function stats(country: CountryCode, ppm: number, gender: GenderLabel, top100Any = false) {
  return { country, ppm, gender, top100_any: top100Any };
}

export const mockCards: Card[] = [
  {
    card_key: 'amelie',
    card_kind: 'group',
    display_name: 'Amélie',
    spellings_preview: ['Amélie', 'Amelie', 'Amélia'],
    score: 11,
    per_country: [
      stats('FR', 520, 'female', false),
      stats('US', 95, 'female', false),
      stats('CL', 88, 'female', false),
    ],
    all_spellings: ['Amélie', 'Amelie', 'Amélia', 'Amelia'],
    within_concept_variants: ['AMELIE'],
    last5_year_ranks: {
      FR: [
        { year: 2020, rank: 84 },
        { year: 2021, rank: 79 },
        { year: 2022, rank: 71 },
        { year: 2023, rank: 68 },
        { year: 2024, rank: 62 },
      ],
      US: [
        { year: 2020, rank: 352 },
        { year: 2021, rank: 340 },
        { year: 2022, rank: 331 },
        { year: 2023, rank: 320 },
        { year: 2024, rank: 310 },
      ],
    },
  },
  {
    card_key: 'aya',
    card_kind: 'concept',
    display_name: 'Aya',
    spellings_preview: ['Aya', 'Ayae', 'Ayaa'],
    score: 9,
    per_country: [
      stats('FR', 140, 'female', false),
      stats('US', 82, 'female', false),
      stats('CL', 40, 'female', false),
    ],
    all_spellings: ['Aya', 'Ayaa', 'Ayae', 'Ayah'],
    within_concept_variants: ['AYA'],
  },
  {
    card_key: 'leo',
    card_kind: 'group',
    display_name: 'Léo',
    spellings_preview: ['Léo', 'Leo', 'Leó'],
    score: 4,
    per_country: [
      stats('FR', 980, 'male', true),
      stats('US', 820, 'male', true),
      stats('CL', 610, 'male', true),
    ],
    all_spellings: ['Léo', 'Leo', 'Leó'],
    within_concept_variants: ['LEO'],
  },
  {
    card_key: 'mila',
    card_kind: 'group',
    display_name: 'Mila',
    spellings_preview: ['Mila', 'Myla', 'Mylah'],
    score: 2,
    per_country: [
      stats('FR', 260, 'female', false),
      stats('US', 1500, 'female', true),
      stats('CL', 210, 'female', false),
    ],
    all_spellings: ['Mila', 'Myla', 'Mylah', 'Milah'],
    within_concept_variants: ['MILA'],
  },
  {
    card_key: 'noa',
    card_kind: 'group',
    display_name: 'Noa',
    spellings_preview: ['Noa', 'Noah', 'Noahh'],
    score: 10,
    per_country: [
      stats('FR', 410, 'unisex', false),
      stats('US', 130, 'male', false),
      stats('CL', 120, 'unisex', false),
    ],
    all_spellings: ['Noa', 'Noah', 'Noahh', 'Noahh'],
    within_concept_variants: ['NOA'],
  },
  {
    card_key: 'sacha',
    card_kind: 'group',
    display_name: 'Sacha',
    spellings_preview: ['Sacha', 'Sasha', 'Sachà'],
    score: 12,
    per_country: [
      stats('FR', 320, 'unisex', false),
      stats('US', 260, 'unisex', false),
      stats('CL', 190, 'unisex', false),
    ],
    all_spellings: ['Sacha', 'Sasha', 'Sachà'],
    within_concept_variants: ['SACHA'],
  },
];
