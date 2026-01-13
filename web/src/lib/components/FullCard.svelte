<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import NameCard from '$lib/components/NameCard.svelte';
  import { api } from '$lib/api';
  import type { Card, CountryCode, GenderLabel } from '$lib/types';

  type Props = {
    initialCard: Card;
    showLastName?: boolean;
    lastName?: string;
    selectedCountries?: CountryCode[];
    showScore?: boolean;
  };

  let {
    initialCard,
    showLastName = true,
    lastName = 'Zarca Wong',
    selectedCountries = ['US', 'FR', 'CL'] as CountryCode[],
    showScore = false,
  }: Props = $props();

  let card = $state<Card | null>(null);
  let flipped = $state(false);
  let showAllSpellings = $state(false);
  let detailsLoading = $state(false);

  let spellingsPage = $state(0);
  const spellingsPageSize = 200;
  let spellingsLoadedPages = $state(0);
  let spellingsHasMore = $state(false);
  let spellingsLoadingPage = $state(false);

  let showRankCounts = $state(false);

  const currentCard = $derived(card ?? initialCard);

  $effect(() => {
    card = null;
    flipped = false;
    showAllSpellings = false;
    showRankCounts = false;
    resetSpellingsPagination();
  });

  function flipToFront() {
    flipped = false;
    showAllSpellings = false;
  }

  function maybeFlipBack(e: MouseEvent) {
    const target = e.target as HTMLElement | null;
    if (!target) return;
    if (target.closest('button,a,input,select,textarea,[role="button"]')) return;
    flipToFront();
  }

  function resetSpellingsPagination() {
    spellingsPage = 0;
    spellingsLoadedPages = 0;
    spellingsHasMore = false;
    spellingsLoadingPage = false;
  }

  async function ensureDetails() {
    if (detailsLoading) return;
    const c = currentCard;
    if (c.all_spellings && c.last5_year_ranks && c.last5_years) return;
    detailsLoading = true;
    try {
      const full = await api.cardDetails(c.card_key, {
        countries: selectedCountries.join(','),
        page: 0,
        page_size: spellingsPageSize,
      });
      card = full;
      spellingsLoadedPages = full.all_spellings ? 1 : 0;
      spellingsHasMore = (full.all_spellings?.length ?? 0) === spellingsPageSize;
    } finally {
      detailsLoading = false;
    }
  }

  async function goToSpellingsPage(nextPage: number) {
    if (!showAllSpellings) return;
    if (nextPage < 0) return;

    if (nextPage < spellingsLoadedPages) {
      spellingsPage = nextPage;
      return;
    }

    if (nextPage !== spellingsLoadedPages) return;
    if (!spellingsHasMore) return;
    if (spellingsLoadingPage) return;
    spellingsLoadingPage = true;
    try {
      const c = currentCard;
      const res = await api.cardDetails(c.card_key, {
        countries: selectedCountries.join(','),
        page: nextPage,
        page_size: spellingsPageSize,
      });
      const existing = c.all_spellings ?? [];
      const next = res.all_spellings ?? [];
      card = { ...c, all_spellings: [...existing, ...next] };
      spellingsLoadedPages = nextPage + 1;
      spellingsPage = nextPage;
      spellingsHasMore = next.length === spellingsPageSize;
    } finally {
      spellingsLoadingPage = false;
    }
  }

  $effect(() => {
    if (!flipped) return;
    void ensureDetails();
  });

  function genderForSelected(card: Card): { gender: GenderLabel; note?: string } {
    const selected = card.per_country.filter((c) => selectedCountries.includes(c.country));
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

  const genderInfo = $derived(genderForSelected(currentCard));
  const tintRgb = $derived(tintRgbForGender(genderInfo.gender));

  function countryLabel(country: CountryCode): string {
    switch (country) {
      case 'US':
        return 'United States';
      case 'FR':
        return 'France';
      case 'CL':
        return 'Chile';
      default:
        return country;
    }
  }

  function rankYearsFor(country: CountryCode): number[] {
    return currentCard.last5_years?.[country] ?? [];
  }

  type RankInfo = { rank: number; total?: number };
  function rankByYearFor(country: CountryCode): Record<string, RankInfo> {
    const m: Record<string, RankInfo> = {};
    for (const r of currentCard.last5_year_ranks?.[country] ?? []) {
      m[String(r.year)] = { rank: r.rank, total: r.total };
    }
    return m;
  }

  type SexSplit = NonNullable<Card['sex_split']>[CountryCode];
  function splitFor(country: CountryCode): SexSplit | undefined {
    return currentCard.sex_split?.[country];
  }
</script>

<div class="aspect-square w-full max-w-md [perspective:1200px]">
  <div
    class={`relative h-full w-full transition-transform duration-300 [transform-style:preserve-3d] ${
      flipped ? '[transform:rotateY(180deg)]' : ''
    }`}
  >
    <div
      class="absolute inset-0 [backface-visibility:hidden]"
      role="button"
      tabindex="0"
      onclick={() => {
        flipped = true;
        showAllSpellings = false;
        resetSpellingsPagination();
      }}
      onkeydown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          flipped = true;
          showAllSpellings = false;
          resetSpellingsPagination();
        }
      }}
    >
      <NameCard
        card={currentCard}
        {showLastName}
        {lastName}
        {selectedCountries}
        {showScore}
        className="h-full w-full"
      />
    </div>

    <div
      class="absolute inset-0 overflow-hidden rounded-3xl border border-zinc-200 bg-white shadow-sm [backface-visibility:hidden] [transform:rotateY(180deg)]"
      role="button"
      tabindex="0"
      onclick={maybeFlipBack}
      onkeydown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          flipToFront();
        }
      }}
    >
      <div
        class="pointer-events-none absolute inset-0"
        style={`background: radial-gradient(circle at 30% 10%, rgba(${tintRgb}, 0.12), transparent 60%);`}
      ></div>

      <div class="relative flex h-full flex-col p-5">
        <div class="flex items-center justify-between gap-3">
          <button
            type="button"
            class="inline-flex items-center gap-2 rounded-full border border-zinc-200 bg-white px-3 py-2 text-sm font-semibold text-zinc-700 shadow-sm hover:bg-zinc-50"
            onclick={() => {
              flipToFront();
            }}
          >
            <Icon name="chevronRight" className="rotate-180" />
            Back
          </button>
          <div class="min-w-0 text-right">
            <div class="truncate text-sm font-semibold text-zinc-900">Details</div>
            <div class="truncate text-xs text-zinc-500">{currentCard.display_name}</div>
          </div>
        </div>

        <div class="mt-4 flex-1 space-y-3 overflow-auto pr-1">
          <section class="rounded-2xl border border-zinc-200 bg-white/80 p-4">
            <div class="flex items-center justify-between gap-3">
              <div class="text-sm font-semibold text-zinc-900">Spellings</div>
              <div class="flex items-center gap-3">
                {#if detailsLoading && !currentCard.all_spellings}
                  <Badge tone="neutral" label="Loading…" />
                {/if}

                {#if currentCard.all_spellings && currentCard.all_spellings.length > 6}
                  <button
                    type="button"
                    class="text-xs font-semibold text-indigo-600 hover:underline"
                    onclick={() => {
                      showAllSpellings = !showAllSpellings;
                      spellingsPage = 0;
                    }}
                  >
                    {showAllSpellings ? 'Show fewer' : 'See all'}
                  </button>
                {/if}
              </div>
            </div>

            {#if showAllSpellings && currentCard.all_spellings}
              <div class="mt-2 flex items-center justify-between text-xs text-zinc-500">
                <div>Page {spellingsPage + 1}</div>
                <div class="flex items-center gap-2">
                  <button
                    type="button"
                    class="rounded-full border border-zinc-200 bg-white px-2 py-1 font-semibold text-zinc-700 disabled:opacity-50"
                    onclick={() => goToSpellingsPage(spellingsPage - 1)}
                    disabled={spellingsPage === 0 || spellingsLoadingPage}
                  >
                    Prev
                  </button>
                  <button
                    type="button"
                    class="rounded-full border border-zinc-200 bg-white px-2 py-1 font-semibold text-zinc-700 disabled:opacity-50"
                    onclick={() => goToSpellingsPage(spellingsPage + 1)}
                    disabled={spellingsLoadingPage ||
                      (!spellingsHasMore && spellingsPage >= spellingsLoadedPages - 1)}
                  >
                    {spellingsLoadingPage ? 'Loading…' : 'Next'}
                  </button>
                </div>
              </div>
            {/if}

            <div class="mt-3 flex flex-wrap gap-2">
              {#if detailsLoading && !currentCard.all_spellings}
                <div class="text-sm text-zinc-600">Fetching spellings…</div>
              {:else}
                {@const all = currentCard.all_spellings ?? currentCard.spellings_preview}
                {@const slice = showAllSpellings
                  ? all.slice(
                      spellingsPage * spellingsPageSize,
                      spellingsPage * spellingsPageSize + spellingsPageSize
                    )
                  : all.slice(0, 10)}
                {#each slice as s (s)}
                  <span
                    class="rounded-full border border-zinc-200 bg-white px-3 py-1 text-xs font-medium text-zinc-700"
                  >
                    {s}
                  </span>
                {/each}
              {/if}
            </div>
            {#if currentCard.within_concept_variants?.length}
              <div class="mt-3 text-xs text-zinc-500">
                Within-concept variants: {currentCard.within_concept_variants.join(', ')}
              </div>
            {/if}
          </section>

          <section class="rounded-2xl border border-zinc-200 bg-white/80 p-4">
            <div class="text-sm font-semibold text-zinc-900">Sex split</div>
            <div class="mt-3 space-y-4">
              {#each selectedCountries as country (country)}
                {@const split = splitFor(country)}
                {@const total = split ? split.male + split.female : 0}
                <div>
                  <div class="flex items-center justify-between text-xs text-zinc-500">
                    <div class="font-semibold">{countryLabel(country)}</div>
                    {#if split && total > 0}
                      <div>
                        Male {Math.round(split.male_pct)}% · Female {Math.round(split.female_pct)}%
                      </div>
                    {:else}
                      <div>—</div>
                    {/if}
                  </div>
                  <div class="mt-2">
                    <div
                      class="relative h-3 w-full rounded-full border border-zinc-200"
                      style="background: linear-gradient(90deg, rgba(59,130,246,0.30) 0%, rgba(236,72,153,0.30) 100%);"
                    >
                      <div class="absolute inset-0 rounded-full bg-white/30"></div>
                      {#if split && total > 0}
                        {@const femalePct = split.female_pct}
                        <div
                          class="absolute top-1/2 h-5 w-[2px] -translate-y-1/2 rounded-full bg-zinc-700/40"
                          style={`left: clamp(0px, calc(${femalePct}% - 1px), calc(100% - 2px));`}
                        ></div>
                        <div
                          class="absolute top-1/2 h-5 w-5 -translate-y-1/2 rounded-full bg-white shadow-sm ring-1 ring-zinc-900/10"
                          style={`left: clamp(0px, calc(${femalePct}% - 10px), calc(100% - 20px));`}
                        ></div>
                      {/if}
                    </div>
                    <div class="mt-1 flex justify-between text-[10px] text-zinc-400">
                      <div>Male</div>
                      <div>Female</div>
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          </section>

          <button
            type="button"
            class="w-full cursor-pointer rounded-2xl border border-zinc-200 bg-white/80 p-4 text-left"
            onclick={() => (showRankCounts = !showRankCounts)}
            aria-label="Toggle rank and occurrences"
          >
            <div class="flex items-center justify-between gap-3">
              <div class="text-sm font-semibold text-zinc-900">
                {showRankCounts ? 'Occurrences (last 5 years)' : 'Rank (last 5 years)'}
              </div>
              <div class="text-xs font-semibold text-indigo-600">
                {showRankCounts ? 'Show ranks' : 'Show counts'}
              </div>
            </div>
            {#if detailsLoading && !currentCard.last5_year_ranks}
              <div class="mt-2 text-sm text-zinc-600">Fetching ranks…</div>
            {:else if currentCard.last5_years}
              <div class="mt-3 space-y-4">
                {#each selectedCountries as country (country)}
                  {@const years = rankYearsFor(country)}
                  {@const byYear = rankByYearFor(country)}
                  {#if years.length}
                    <div>
                      <div class="text-xs font-semibold text-zinc-500">{countryLabel(country)}</div>
                      <div class="mt-2 grid grid-cols-5 gap-2 text-center text-[11px]">
                        {#each years as year (year)}
                          {@const info = byYear[String(year)]}
                          <div class="rounded-xl border border-zinc-200 bg-white p-2">
                            <div class="text-zinc-500">{year}</div>
                            <div class="mt-1 font-semibold">
                              {#if showRankCounts}
                                {info?.total !== undefined
                                  ? new Intl.NumberFormat().format(info.total)
                                  : '—'}
                              {:else}
                                {info?.rank !== undefined ? `#${info.rank}` : '—'}
                              {/if}
                            </div>
                          </div>
                        {/each}
                      </div>
                    </div>
                  {/if}
                {/each}
              </div>
            {:else}
              <div class="mt-2 text-sm text-zinc-600">Not available.</div>
            {/if}
          </button>
        </div>

        {#if genderInfo.note}
          <div class="mt-3 text-center text-[11px] text-zinc-400">{genderInfo.note}</div>
        {/if}
      </div>
    </div>
  </div>
</div>
