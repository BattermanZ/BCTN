<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import type { Card, CountryCode, GenderLabel, PerCountryStats } from '$lib/types';

  type Props = {
    card: Card;
    showLastName?: boolean;
    lastName?: string;
    selectedCountries?: CountryCode[];
    showScore?: boolean;
  };

  let {
    card,
    showLastName = true,
    lastName = 'Zarca Wong',
    selectedCountries = ['US', 'FR', 'CL'] as CountryCode[],
    showScore = false,
  }: Props = $props();

  function badgeForGender(label: GenderLabel) {
    switch (label) {
      case 'female':
        return { tone: 'indigo' as const, label: 'Female' };
      case 'male':
        return { tone: 'indigo' as const, label: 'Male' };
      case 'mostly_female':
        return { tone: 'neutral' as const, label: 'Mostly female' };
      case 'mostly_male':
        return { tone: 'neutral' as const, label: 'Mostly male' };
      case 'unisex':
        return { tone: 'teal' as const, label: 'Unisex' };
      default:
        return { tone: 'neutral' as const, label: 'Unknown' };
    }
  }

  function genderForSelected(): { gender: GenderLabel; note?: string } {
    const selected = card.per_country.filter((c: PerCountryStats) =>
      selectedCountries.includes(c.country)
    );
    if (selected.some((c) => c.gender === 'unisex')) {
      const countries = selected
        .filter((c) => c.gender === 'unisex')
        .map((c) => c.country)
        .join(', ');
      return { gender: 'unisex', note: `Unisex in ${countries}` };
    }
    const first = selected[0] ?? card.per_country[0];
    return { gender: first?.gender ?? 'unknown', note: undefined };
  }

  function fmtPpm(ppm: number) {
    if (ppm <= 0) return '—';
    if (ppm >= 1000) return `${Math.round(ppm)}`;
    return ppm.toFixed(0);
  }

  const gender = $derived(genderForSelected());
  const genderBadge = $derived(badgeForGender(gender.gender));
</script>

<div
  class="rounded-3xl border border-zinc-200 bg-white p-5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900/40"
>
  <div class="flex items-start justify-between gap-3">
    <div class="min-w-0">
      <div class="text-3xl font-semibold tracking-tight text-zinc-900 dark:text-zinc-50">
        {card.display_name}
      </div>
      {#if showLastName}
        <div class="mt-1 text-sm text-zinc-500 dark:text-zinc-300">
          {card.display_name}
          {lastName}
        </div>
      {:else}
        <div class="mt-1 text-sm text-zinc-500 dark:text-zinc-300">First name only</div>
      {/if}
    </div>

    {#if showScore && card.score !== undefined}
      <Badge tone="amber" label={`Score ${card.score}`} />
    {/if}
  </div>

  <div class="mt-4 flex flex-wrap gap-2">
    {#each card.spellings_preview.slice(0, 3) as spelling (spelling)}
      <span
        class="rounded-full border border-zinc-200 bg-zinc-50 px-3 py-1 text-sm text-zinc-700 dark:border-zinc-800 dark:bg-zinc-950/50 dark:text-zinc-200"
      >
        {spelling}
      </span>
    {/each}
  </div>

  <div class="mt-4 flex flex-wrap items-center gap-2">
    <Badge tone={genderBadge.tone} label={genderBadge.label} />
    {#if gender.note}
      <span class="text-xs text-zinc-500 dark:text-zinc-300">{gender.note}</span>
    {/if}
  </div>

  <div class="mt-4 flex flex-wrap gap-2">
    {#each card.per_country as c (c.country)}
      {#if selectedCountries.includes(c.country)}
        <Badge
          tone={c.top100_any ? 'rose' : 'neutral'}
          label={`${c.country} ${fmtPpm(c.ppm)} ppm`}
        />
      {/if}
    {/each}
  </div>

  <div class="mt-5 text-xs text-zinc-500 dark:text-zinc-300">
    Swipe right to Like • left to Nope • up to Super-like • tap for details
  </div>
</div>
