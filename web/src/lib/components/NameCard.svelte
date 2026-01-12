<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import type { Card, CountryCode, GenderLabel, PerCountryStats } from '$lib/types';

  type Props = {
    card: Card;
    showLastName?: boolean;
    lastName?: string;
    selectedCountries?: CountryCode[];
    showScore?: boolean;
    className?: string;
  };

  let {
    card,
    showLastName = true,
    lastName = 'Zarca Wong',
    selectedCountries = ['US', 'FR', 'CL'] as CountryCode[],
    showScore = false,
    className = '',
  }: Props = $props();

  function badgeForGender(label: GenderLabel) {
    switch (label) {
      case 'female':
        return { tone: 'pink' as const, label: 'Female' };
      case 'male':
        return { tone: 'blue' as const, label: 'Male' };
      case 'mostly_female':
        return { tone: 'pink' as const, label: 'Mostly female' };
      case 'mostly_male':
        return { tone: 'blue' as const, label: 'Mostly male' };
      case 'unisex':
        return { tone: 'violet' as const, label: 'Unisex' };
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

  function latestRank(country: CountryCode): { year: number; rank: number } | undefined {
    const ranks = card.last5_year_ranks?.[country];
    if (!ranks || !ranks.length) return undefined;
    return ranks.reduce((best, cur) => (cur.year > best.year ? cur : best));
  }

  const gender = $derived(genderForSelected());
  const genderBadge = $derived(badgeForGender(gender.gender));

  function tintRgbForGender(label: GenderLabel): string {
    switch (label) {
      case 'male':
      case 'mostly_male':
        return '59,130,246'; // blue-500
      case 'female':
      case 'mostly_female':
        return '236,72,153'; // pink-500
      case 'unisex':
        return '139,92,246'; // violet-500
      default:
        return '99,102,241'; // indigo-500
    }
  }

  const tintRgb = $derived(tintRgbForGender(gender.gender));
</script>

<div
  class={`relative overflow-hidden rounded-3xl border border-zinc-200 bg-white shadow-sm ${className}`}
>
  <div
    class="pointer-events-none absolute inset-0"
    style={`background: radial-gradient(circle at 30% 10%, rgba(${tintRgb}, 0.10), transparent 55%);`}
  ></div>

  <div class="relative flex h-full flex-col p-6">
    <div class="flex items-start justify-between gap-6">
      <div class="min-w-0">
        {#if showScore && card.score !== undefined}
          <Badge tone="amber" label={`Score ${card.score}`} />
        {/if}
      </div>
      <div class="shrink-0">
        <Badge tone={genderBadge.tone} label={genderBadge.label} />
      </div>
    </div>

    <div class="flex flex-1 flex-col items-center text-center">
      <div class="mt-1 flex flex-wrap justify-center gap-2">
        {#each card.spellings_preview.slice(0, 3) as spelling (spelling)}
          <span
            class="rounded-full border border-zinc-200 bg-white/80 px-3 py-1 text-xs font-medium text-zinc-700"
          >
            {spelling}
          </span>
        {/each}
      </div>

      <div class="flex flex-1 flex-col items-center justify-center">
        <div class="text-5xl font-semibold tracking-tight text-zinc-900">
          {card.display_name}
        </div>

        {#if showLastName}
          <div class="mt-10 text-lg font-medium text-zinc-600">
            {`${card.display_name} ${lastName}`.trim()}
          </div>
        {/if}
      </div>
    </div>

    <div class="flex flex-wrap items-center justify-center gap-2 pt-2">
      {#each card.per_country as c (c.country)}
        {#if selectedCountries.includes(c.country)}
          {@const r = latestRank(c.country)}
          <Badge
            tone={c.top100_any ? 'rose' : 'neutral'}
            label={r ? `${c.country} #${r.rank}` : `${c.country} —`}
          />
        {/if}
      {/each}
    </div>

    {#if gender.note}
      <div class="mt-2 text-center text-[11px] text-zinc-400">{gender.note}</div>
    {/if}
  </div>
</div>
