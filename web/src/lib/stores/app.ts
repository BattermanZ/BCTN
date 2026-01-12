import { writable } from 'svelte/store';
import type { CountryCode, SwipeAction, User } from '$lib/types';

export type AppFilters = {
  countries: CountryCode[];
  popularity: 'rare' | 'common' | 'top';
  includeExtraRare: boolean;
  gender: 'any' | 'unisex' | 'mostly_male' | 'mostly_female' | 'male' | 'female';
};

export type AppSettings = {
  showLastName: boolean;
  lastName: string;
  bestPickMode: boolean;
};

export type SwipeState = {
  byCardKey: Record<string, SwipeAction>;
  lastAction?: { card_key: string; action: SwipeAction };
};

export const currentUser = writable<User | null>(null);
export const partnerUser = writable<User | null>(null);

export const filters = writable<AppFilters>({
  countries: ['US', 'FR', 'CL'],
  popularity: 'common',
  includeExtraRare: false,
  gender: 'any',
});

export const settings = writable<AppSettings>({
  showLastName: true,
  lastName: 'Zarca Wong',
  bestPickMode: false,
});

export const swipes = writable<SwipeState>({
  byCardKey: {},
});
