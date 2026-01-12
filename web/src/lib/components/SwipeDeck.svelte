<script lang="ts">
  import BottomSheet from '$lib/components/BottomSheet.svelte';
  import NameCard from '$lib/components/NameCard.svelte';
  import type { Card, CountryCode, SwipeAction } from '$lib/types';
  import { filters, settings, swipes } from '$lib/stores/app';
  import { fade } from 'svelte/transition';

  type Props = { cards: Card[] };
  let { cards }: Props = $props();

  let index = $state(0);
  let sheetOpen = $state(false);
  let showAllSpellings = $state(false);

  let dragging = $state(false);
  let startX = $state(0);
  let startY = $state(0);
  let dx = $state(0);
  let dy = $state(0);

  const current = $derived(cards[index]);
  const remaining = $derived(cards.length - index);

  function resetDrag() {
    dragging = false;
    startX = 0;
    startY = 0;
    dx = 0;
    dy = 0;
  }

  function onPointerDown(e: PointerEvent) {
    if (!current) return;
    dragging = true;
    startX = e.clientX;
    startY = e.clientY;
    dx = 0;
    dy = 0;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    dx = e.clientX - startX;
    dy = e.clientY - startY;
  }

  function onPointerUp() {
    if (!dragging) return;

    const absX = Math.abs(dx);
    const absY = Math.abs(dy);

    const likeThreshold = 110;
    const superLikeThreshold = 90;

    if (absX > absY && absX > likeThreshold) {
      void commitSwipe(dx > 0 ? 'like' : 'nope');
    } else if (-dy > superLikeThreshold && absY > absX) {
      void commitSwipe('superlike');
    } else {
      resetDrag();
    }
  }

  function swipeLabel(): { label: string; tone: 'neutral' | 'teal' | 'rose' | 'amber' } {
    if (!dragging) return { label: '', tone: 'neutral' };
    if (dx > 60) return { label: 'LIKE', tone: 'teal' };
    if (dx < -60) return { label: 'NOPE', tone: 'rose' };
    if (dy < -45) return { label: 'SUPER', tone: 'amber' };
    return { label: '', tone: 'neutral' };
  }

  const label = $derived(swipeLabel());

  async function commitSwipe(action: SwipeAction) {
    if (!current) return;
    const cardKey = current.card_key;

    swipes.update((s) => ({
      ...s,
      byCardKey: { ...s.byCardKey, [cardKey]: action },
      lastAction: { card_key: cardKey, action },
    }));

    // animate off-screen quickly
    if (action === 'superlike') {
      dy = -420;
      dx = 0;
    } else {
      dx = action === 'like' ? 420 : -420;
      dy = 0;
    }
    dragging = false;

    await new Promise((r) => setTimeout(r, 140));
    index = Math.min(index + 1, cards.length);
    resetDrag();
  }

  function undo() {
    swipes.update((s) => {
      const last = s.lastAction;
      if (!last) return s;
      const nextBy = { ...s.byCardKey };
      delete nextBy[last.card_key];
      return { ...s, byCardKey: nextBy, lastAction: undefined };
    });
    if (index > 0) {
      index -= 1;
    }
  }

  function openDetails() {
    if (!current) return;
    sheetOpen = true;
    showAllSpellings = false;
  }

  type RankPoint = { year: number; rank: number };
  function rankEntries(input: Card['last5_year_ranks']): Array<[CountryCode, Array<RankPoint>]> {
    if (!input) return [];
    return (Object.entries(input) as Array<[CountryCode, Array<RankPoint> | undefined]>)
      .filter(([, ranks]) => Array.isArray(ranks) && ranks.length > 0)
      .map(([country, ranks]) => [country, ranks as Array<RankPoint>]);
  }
</script>

{#if !current}
  <div
    class="rounded-3xl border border-zinc-200 bg-white p-6 text-center dark:border-zinc-800 dark:bg-zinc-900/40"
  >
    <div class="text-lg font-semibold">No more names</div>
    <div class="mt-2 text-sm text-zinc-600 dark:text-zinc-300">
      Try broadening filters (countries, gender, popularity) to keep exploring.
    </div>
  </div>
{:else}
  <div class="flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <div class="text-sm text-zinc-500 dark:text-zinc-300">{remaining} left</div>
      <button
        type="button"
        class="rounded-full border border-zinc-200 bg-white px-3 py-1.5 text-sm font-medium text-zinc-700 hover:bg-zinc-50 dark:border-zinc-800 dark:bg-zinc-950 dark:text-zinc-200 dark:hover:bg-zinc-900"
        onclick={undo}
      >
        Undo
      </button>
    </div>

    <div class="relative">
      <!-- next card placeholder -->
      {#if cards[index + 1]}
        <div class="pointer-events-none absolute inset-0 translate-y-2 scale-[0.98] opacity-60">
          <NameCard
            card={cards[index + 1]}
            showLastName={$settings.showLastName}
            lastName={$settings.lastName}
            selectedCountries={$filters.countries}
            showScore={$settings.bestPickMode}
          />
        </div>
      {/if}

      <div
        class="relative touch-pan-y"
        style={`transform: translate3d(${dx}px, ${dy}px, 0) rotate(${dx / 18}deg); transition: ${
          dragging ? 'none' : 'transform 140ms cubic-bezier(0.2, 0.8, 0.2, 1)'
        };`}
        onpointerdown={onPointerDown}
        onpointermove={onPointerMove}
        onpointerup={onPointerUp}
        onpointercancel={onPointerUp}
        onkeydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            openDetails();
          }
        }}
        role="button"
        tabindex="0"
        onclick={openDetails}
      >
        <NameCard
          card={current}
          showLastName={$settings.showLastName}
          lastName={$settings.lastName}
          selectedCountries={$filters.countries}
          showScore={$settings.bestPickMode}
        />

        {#if label.label}
          <div
            class={`pointer-events-none absolute left-6 top-6 rounded-2xl border px-3 py-1 text-sm font-bold tracking-wide ${
              label.tone === 'teal'
                ? 'border-teal-300 bg-teal-50 text-teal-800 dark:border-teal-700 dark:bg-teal-950/50 dark:text-teal-200'
                : label.tone === 'rose'
                  ? 'border-rose-300 bg-rose-50 text-rose-800 dark:border-rose-700 dark:bg-rose-950/50 dark:text-rose-200'
                  : 'border-amber-300 bg-amber-50 text-amber-900 dark:border-amber-700 dark:bg-amber-950/50 dark:text-amber-200'
            }`}
            transition:fade={{ duration: 80 }}
          >
            {label.label}
          </div>
        {/if}
      </div>
    </div>

    <div class="grid grid-cols-3 gap-3 pt-2">
      <button
        type="button"
        class="rounded-full border border-rose-200 bg-rose-50 px-4 py-3 text-sm font-semibold text-rose-800 shadow-sm hover:bg-rose-100 dark:border-rose-900/60 dark:bg-rose-950/40 dark:text-rose-200 dark:hover:bg-rose-950/70"
        onclick={() => commitSwipe('nope')}
      >
        Nope
      </button>
      <button
        type="button"
        class="rounded-full border border-teal-200 bg-teal-50 px-4 py-3 text-sm font-semibold text-teal-800 shadow-sm hover:bg-teal-100 dark:border-teal-900/60 dark:bg-teal-950/40 dark:text-teal-200 dark:hover:bg-teal-950/70"
        onclick={() => commitSwipe('like')}
      >
        Like
      </button>
      <button
        type="button"
        class="rounded-full border border-amber-200 bg-amber-50 px-4 py-3 text-sm font-semibold text-amber-900 shadow-sm hover:bg-amber-100 dark:border-amber-900/60 dark:bg-amber-950/40 dark:text-amber-200 dark:hover:bg-amber-950/70"
        onclick={() => commitSwipe('superlike')}
      >
        Super
      </button>
    </div>
  </div>
{/if}

<BottomSheet open={sheetOpen} title={current?.display_name} onClose={() => (sheetOpen = false)}>
  {#if current}
    <div class="space-y-5">
      <div
        class="rounded-2xl border border-zinc-200 bg-zinc-50 p-4 dark:border-zinc-800 dark:bg-zinc-900/30"
      >
        <div class="text-sm font-semibold">Spellings</div>
        <div class="mt-2 flex flex-wrap gap-2">
          {#each current.all_spellings?.slice(0, showAllSpellings ? 50 : 6) ?? current.spellings_preview as s (s)}
            <span
              class="rounded-full border border-zinc-200 bg-white px-3 py-1 text-sm text-zinc-700 dark:border-zinc-800 dark:bg-zinc-950 dark:text-zinc-200"
            >
              {s}
            </span>
          {/each}
        </div>
        {#if current.all_spellings && current.all_spellings.length > 6}
          <button
            type="button"
            class="mt-3 text-sm font-medium text-indigo-600 hover:underline dark:text-indigo-400"
            onclick={() => (showAllSpellings = !showAllSpellings)}
          >
            {showAllSpellings ? 'Show fewer' : 'See all spellings'}
          </button>
        {/if}

        {#if current.within_concept_variants?.length}
          <div class="mt-4 text-xs text-zinc-500 dark:text-zinc-300">
            Within-concept variants: {current.within_concept_variants.join(', ')}
          </div>
        {/if}
      </div>

      <div
        class="rounded-2xl border border-zinc-200 bg-zinc-50 p-4 dark:border-zinc-800 dark:bg-zinc-900/30"
      >
        <div class="text-sm font-semibold">Popularity (per million births)</div>
        <div class="mt-3 space-y-2">
          {#each current.per_country as c (c.country)}
            <div class="flex items-center justify-between text-sm">
              <div class="font-medium">{c.country}</div>
              <div class="text-zinc-600 dark:text-zinc-200">{Math.round(c.ppm)} ppm</div>
            </div>
          {/each}
        </div>
      </div>

      <div
        class="rounded-2xl border border-zinc-200 bg-zinc-50 p-4 dark:border-zinc-800 dark:bg-zinc-900/30"
      >
        <div class="text-sm font-semibold">Rank (last 5 years)</div>
        {#if current.last5_year_ranks}
          <div class="mt-3 space-y-4">
            {#each rankEntries(current.last5_year_ranks) as [country, ranks] (country)}
              <div>
                <div class="text-xs font-semibold text-zinc-500 dark:text-zinc-300">{country}</div>
                <div class="mt-2 grid grid-cols-5 gap-2 text-center text-xs">
                  {#each ranks as r (r.year)}
                    <div
                      class="rounded-xl border border-zinc-200 bg-white p-2 dark:border-zinc-800 dark:bg-zinc-950"
                    >
                      <div class="text-zinc-500 dark:text-zinc-300">{r.year}</div>
                      <div class="mt-1 font-semibold">#{r.rank}</div>
                    </div>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        {:else}
          <div class="mt-2 text-sm text-zinc-600 dark:text-zinc-300">
            Not available in mock data.
          </div>
        {/if}
      </div>
    </div>
  {/if}
</BottomSheet>
