<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import NameCard from '$lib/components/NameCard.svelte';
  import type { Card, CountryCode, GenderLabel, SwipeAction } from '$lib/types';
  import { api } from '$lib/api';
  import { filters, settings, swipes } from '$lib/stores/app';
  import { goto } from '$app/navigation';
  import { fade } from 'svelte/transition';
  import { untrack } from 'svelte';

  let deck = $state<Card[]>([]);
  let showAllSpellings = $state(false);
  let flipped = $state(false);
  let loading = $state(false);
  let loadError = $state<string | null>(null);
  let detailsLoading = $state(false);

  let feedGen = $state(0);

  let spellingsPage = $state(0);
  const spellingsPageSize = 200;
  let spellingsLoadedPages = $state(0);
  let spellingsHasMore = $state(false);
  let spellingsLoadingPage = $state(false);

  let showRankCounts = $state(false);

  let dragging = $state(false);
  let activePointerId = $state<number | null>(null);
  let startX = $state(0);
  let startY = $state(0);
  let dx = $state(0);
  let dy = $state(0);

  const topCard = $derived(deck[0]);
  const midCard = $derived(deck[1]);
  const backCard = $derived(deck[2]);
  const targetDeckSize = 10;

  type MoveSample = { t: number; dx: number; dy: number };
  let samples = $state<MoveSample[]>([]);

  let animating = $state(false);
  let animX = $state(0);
  let animY = $state(0);
  let rafId = $state<number | null>(null);
  let promoteT = $state(0);
  let promoteRafId = $state<number | null>(null);

  let history = $state<Array<{ card: Card; action: SwipeAction }>>([]);

  const topX = $derived(dragging ? dx : animating ? animX : 0);
  const topY = $derived(dragging ? dy : animating ? animY : 0);
  const topRotate = $derived(topX / 18);

  const activeFeedKey = $derived(
    `${$settings.bestPickMode ? 1 : 0}|${$filters.countries.join(',')}|${$filters.popularity.join(
      ','
    )}|${$filters.gender}`
  );
  let lastFeedKey = $state<string | null>(null);

  async function refillDeck() {
    if (loading) return;
    loadError = null;
    loading = true;
    const gen = feedGen;
    try {
      while (deck.length < targetDeckSize) {
        if (gen != feedGen) return;
        const missing = targetDeckSize - deck.length;
        const batch = await api.feedBatch({
          countries: $filters.countries.join(','),
          pop: $filters.popularity.join(','),
          gender: $filters.gender,
          limit: Math.min(10, missing),
        });
        if (gen != feedGen) return;
        if (!batch.length) {
          break;
        }
        deck = [...deck, ...batch];
      }
    } catch (e) {
      if (gen != feedGen) return;
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
        return;
      }
      loadError = (e as Error | null)?.message ?? 'Failed to load names';
    } finally {
      if (gen == feedGen) loading = false;
    }
  }

  function resetSpellingsPagination() {
    spellingsPage = 0;
    spellingsLoadedPages = 0;
    spellingsHasMore = false;
    spellingsLoadingPage = false;
  }

  function resetDeck() {
    feedGen += 1;
    resetDrag();
    resetAnimation();
    flipped = false;
    showAllSpellings = false;
    history = [];
    deck = [];
    loading = false;
    detailsLoading = false;
    resetSpellingsPagination();
    void refillDeck();
  }

  $effect(() => {
    const key = activeFeedKey;
    if (lastFeedKey === key) return;
    lastFeedKey = key;
    // restart the feed when filters/best-pick change (per product decision)
    untrack(() => resetDeck());
  });

  async function ensureTopDetails(card: Card) {
    if (detailsLoading) return;
    if (card.all_spellings && card.last5_year_ranks) return;
    detailsLoading = true;
    try {
      const full = await api.cardDetails(card.card_key, {
        countries: $filters.countries.join(','),
        page: 0,
        page_size: spellingsPageSize,
      });
      if (deck[0]?.card_key === card.card_key) {
        deck = [full, ...deck.slice(1)];
        spellingsPage = 0;
        spellingsLoadedPages = full.all_spellings ? 1 : 0;
        spellingsHasMore = (full.all_spellings?.length ?? 0) === spellingsPageSize;
      }
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
      }
    } finally {
      detailsLoading = false;
    }
  }

  async function goToSpellingsPage(nextPage: number) {
    if (!topCard) return;
    if (!showAllSpellings) return;
    if (nextPage < 0) return;

    // If already loaded, just switch pages.
    if (nextPage < spellingsLoadedPages) {
      spellingsPage = nextPage;
      return;
    }

    // Only support sequential paging (avoid skipping pages).
    if (nextPage !== spellingsLoadedPages) {
      return;
    }
    if (!spellingsHasMore) {
      return;
    }
    if (spellingsLoadingPage) return;
    spellingsLoadingPage = true;
    try {
      const res = await api.cardDetails(topCard.card_key, {
        countries: $filters.countries.join(','),
        page: nextPage,
        page_size: spellingsPageSize,
      });
      if (deck[0]?.card_key !== topCard.card_key) return;

      const existing = deck[0].all_spellings ?? [];
      const next = res.all_spellings ?? [];
      deck = [{ ...deck[0], all_spellings: [...existing, ...next] }, ...deck.slice(1)];

      spellingsLoadedPages = nextPage + 1;
      spellingsPage = nextPage;
      spellingsHasMore = next.length === spellingsPageSize;
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
      }
    } finally {
      spellingsLoadingPage = false;
    }
  }

  $effect(() => {
    if (!flipped || !topCard) return;
    void ensureTopDetails(topCard);
  });

  function resetDrag() {
    dragging = false;
    activePointerId = null;
    startX = 0;
    startY = 0;
    dx = 0;
    dy = 0;
    samples = [];
  }

  function resetAnimation() {
    animating = false;
    animX = 0;
    animY = 0;
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
  }

  function startPromotion(durationMs: number) {
    if (promoteRafId !== null) {
      cancelAnimationFrame(promoteRafId);
      promoteRafId = null;
    }

    const start = performance.now();
    promoteT = 0;
    const step = () => {
      const t = Math.min(1, (performance.now() - start) / durationMs);
      promoteT = 1 - Math.pow(1 - t, 3);
      const done = t >= 1;
      if (done) {
        promoteRafId = null;
        return;
      }
      promoteRafId = requestAnimationFrame(step);
    };
    promoteRafId = requestAnimationFrame(step);
  }

  $effect(() => {
    return () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      if (promoteRafId !== null) cancelAnimationFrame(promoteRafId);
    };
  });

  function onPointerDown(e: PointerEvent) {
    if (!topCard) return;
    if (dragging) return;
    if (animating) return;
    if (flipped) return;
    // iOS Safari PWA can scroll the page unless we actively suppress default touch behavior.
    if (e.cancelable) e.preventDefault();
    dragging = true;
    activePointerId = e.pointerId;
    startX = e.clientX;
    startY = e.clientY;
    dx = 0;
    dy = 0;
    samples = [{ t: performance.now(), dx: 0, dy: 0 }];
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    if (activePointerId !== e.pointerId) return;
    if (e.cancelable) e.preventDefault();
    dx = e.clientX - startX;
    dy = e.clientY - startY;

    const now = performance.now();
    samples = [...samples, { t: now, dx, dy }].filter((s) => now - s.t <= 120).slice(-6);
  }

  function onPointerUp(e: PointerEvent) {
    if (!dragging) return;
    if (activePointerId !== e.pointerId) return;
    if (e.cancelable) e.preventDefault();

    const absX = Math.abs(dx);
    const absY = Math.abs(dy);

    const likeThreshold = 110;
    const superLikeThreshold = 90;
    const tapThreshold = 8;

    if (absX > absY && absX > likeThreshold) {
      const action: SwipeAction = dx > 0 ? 'like' : 'nope';
      void commitSwipeWithMomentum(action, { startDx: dx, startDy: dy });
    } else if (-dy > superLikeThreshold && absY > absX) {
      void commitSwipeWithMomentum('superlike', { startDx: dx, startDy: dy });
    } else {
      if (absX < tapThreshold && absY < tapThreshold) {
        flipped = !flipped;
        showAllSpellings = false;
      }
      resetDrag();
    }
  }

  function onPointerCancel(e: PointerEvent) {
    if (activePointerId !== e.pointerId) return;
    resetDrag();
  }

  function suppressTouchScroll(node: HTMLElement) {
    const onTouchStart = (e: TouchEvent) => {
      if (flipped) return;
      if (animating) return;
      if (e.cancelable) e.preventDefault();
    };

    const onTouchMove = (e: TouchEvent) => {
      if (flipped) return;
      if (e.cancelable) e.preventDefault();
    };

    node.addEventListener('touchstart', onTouchStart, { passive: false });
    node.addEventListener('touchmove', onTouchMove, { passive: false });

    return {
      destroy() {
        node.removeEventListener('touchstart', onTouchStart);
        node.removeEventListener('touchmove', onTouchMove);
      },
    };
  }

  function swipeLabel(): { label: string; tone: 'neutral' | 'teal' | 'rose' | 'amber' } {
    if (!dragging) return { label: '', tone: 'neutral' };
    if (dx > 60) return { label: 'LIKE', tone: 'teal' };
    if (dx < -60) return { label: 'NOPE', tone: 'rose' };
    if (dy < -45) return { label: 'SUPER', tone: 'amber' };
    return { label: '', tone: 'neutral' };
  }

  const label = $derived(swipeLabel());

  function sampleVelocity(): { vx: number; vy: number } {
    if (samples.length < 2) return { vx: 0, vy: 0 };
    const a = samples[samples.length - 2];
    const b = samples[samples.length - 1];
    const dt = Math.max(1, b.t - a.t);
    return { vx: (b.dx - a.dx) / dt, vy: (b.dy - a.dy) / dt };
  }

  function targetForAction(
    action: SwipeAction,
    startDx: number,
    startDy: number,
    vx: number,
    vy: number
  ): { x: number; y: number; durationMs: number } {
    const vw = Math.max(320, window.innerWidth || 320);
    const vh = Math.max(480, window.innerHeight || 480);

    const boostX = Math.min(520, Math.abs(vx) * 900);
    const boostY = Math.min(520, Math.abs(vy) * 900);

    if (action === 'superlike') {
      const y = -vh * 1.1 - boostY;
      const x = startDx + vx * 220;
      return { x, y, durationMs: 260 };
    }

    const dir = action === 'like' ? 1 : -1;
    const x = dir * (vw * 1.1 + boostX);
    const y = startDy + vy * 220;
    return { x, y, durationMs: 220 };
  }

  function animateTo(targetX: number, targetY: number, durationMs: number): Promise<void> {
    const fromX = animX;
    const fromY = animY;
    const start = performance.now();

    return new Promise((resolve) => {
      const step = (now: number) => {
        const t = Math.min(1, (now - start) / durationMs);
        const ease = 1 - Math.pow(1 - t, 3);
        animX = fromX + (targetX - fromX) * ease;
        animY = fromY + (targetY - fromY) * ease;

        if (t < 1) {
          rafId = requestAnimationFrame(step);
          return;
        }
        rafId = null;
        resolve();
      };
      rafId = requestAnimationFrame(step);
    });
  }

  async function commitSwipeWithMomentum(
    action: SwipeAction,
    opts: { startDx: number; startDy: number }
  ) {
    if (!topCard) return;
    if (animating) return;
    flipped = false;

    const swipedCard = topCard;
    const cardKey = swipedCard.card_key;
    const { vx, vy } = sampleVelocity();

    // lock gesture immediately
    dragging = false;
    activePointerId = null;

    // record swipe immediately (even while animating)
    history = [...history, { card: swipedCard, action }];
    swipes.update((s) => ({
      ...s,
      byCardKey: { ...s.byCardKey, [cardKey]: action },
      lastAction: { card_key: cardKey, action },
    }));
    void api.createSwipe({ card_key: cardKey, card_kind: swipedCard.card_kind, action });

    // animate top card off-screen
    resetAnimation();
    animating = true;
    animX = opts.startDx;
    animY = opts.startDy;
    dx = 0;
    dy = 0;
    samples = [];

    const target = targetForAction(action, animX, animY, vx, vy);
    startPromotion(target.durationMs);
    await animateTo(target.x, target.y, target.durationMs);

    // advance the deck only after the top card has left the screen
    deck = deck.slice(1);
    resetAnimation();
    showAllSpellings = false;
    resetSpellingsPagination();
    await refillDeck();
    // reset the stack back to its resting (stacked) state immediately after advancing
    if (promoteRafId !== null) {
      cancelAnimationFrame(promoteRafId);
      promoteRafId = null;
    }
    promoteT = 0;
  }

  async function undo() {
    if (dragging || animating) return;
    const last = history.at(-1);
    if (!last) return;
    flipped = false;
    showAllSpellings = false;

    try {
      const res = await api.undoSwipe();
      if (res.status !== 'ok' || !res.undone) return;

      // Restore the last swiped card locally (prefer our history; fallback to API lookup).
      const undoneKey = res.undone.card_key;
      const card =
        last.card.card_key === undoneKey ? last.card : await api.cardDetails(res.undone.card_key);

      history = history.slice(0, -1);
      swipes.update((s) => {
        const nextBy = { ...s.byCardKey };
        delete nextBy[undoneKey];
        return { ...s, byCardKey: nextBy, lastAction: undefined };
      });
      deck = [card, ...deck];
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
      }
    }
  }

  function canSwipe(): boolean {
    return Boolean(topCard) && !dragging && !animating && !flipped;
  }

  function genderForSelected(card: Card): { gender: GenderLabel; note?: string } {
    const selected = card.per_country.filter((c) => $filters.countries.includes(c.country));
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

  const topGenderInfo = $derived(
    topCard ? genderForSelected(topCard) : ({ gender: 'unknown', note: undefined } as const)
  );
  const topTintRgb = $derived(tintRgbForGender(topGenderInfo.gender));

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
    return topCard?.last5_years?.[country] ?? [];
  }

  type RankInfo = { rank: number; total?: number };
  function rankByYearFor(country: CountryCode): Record<string, RankInfo> {
    const m: Record<string, RankInfo> = {};
    for (const r of topCard?.last5_year_ranks?.[country] ?? []) {
      m[String(r.year)] = { rank: r.rank, total: r.total };
    }
    return m;
  }

  type SexSplit = NonNullable<Card['sex_split']>[CountryCode];
  function splitFor(country: CountryCode): SexSplit | undefined {
    return topCard?.sex_split?.[country];
  }
</script>

{#if !topCard}
  {#if loading}
    <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-center">
      <div class="text-lg font-semibold">Loading…</div>
      <div class="mt-2 text-sm text-zinc-600">Fetching more names.</div>
    </div>
  {:else}
    <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-center">
      <div class="text-lg font-semibold">No more names</div>
      <div class="mt-2 text-sm text-zinc-600">
        Try broadening filters (countries, gender, popularity) to keep exploring.
      </div>
    </div>
  {/if}
{:else}
  <div class="flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <div class="text-sm text-zinc-500">
        {#if loadError}
          {loadError}
        {:else if loading}
          Loading…
        {:else}
          {Math.max(0, deck.length - 1)} queued
        {/if}
      </div>
      <button
        type="button"
        class="rounded-full border border-zinc-200 bg-white px-3 py-1.5 text-sm font-medium text-zinc-700 hover:bg-zinc-50"
        onclick={undo}
        disabled={history.length === 0 || dragging || animating}
      >
        Undo
      </button>
    </div>

    <div class="relative flex items-center justify-center">
      <div
        class="relative aspect-square w-[min(92vw,420px)] touch-none select-none overscroll-contain"
      >
        <!-- back -->
        {#if backCard}
          <div
            class="pointer-events-none absolute inset-0 z-0"
            style={`transform: translate3d(0, ${18 - promoteT * 8}px, 0) scale(${0.94 + promoteT * 0.03});`}
          >
            <NameCard
              card={backCard}
              showLastName={$settings.showLastName}
              lastName={$settings.lastName}
              selectedCountries={$filters.countries}
              showScore={$settings.bestPickMode}
              className="h-full w-full"
            />
          </div>
        {/if}

        <!-- middle -->
        {#if midCard}
          <div
            class="pointer-events-none absolute inset-0 z-10"
            style={`transform: translate3d(0, ${10 - promoteT * 10}px, 0) scale(${0.97 + promoteT * 0.03});`}
          >
            <NameCard
              card={midCard}
              showLastName={$settings.showLastName}
              lastName={$settings.lastName}
              selectedCountries={$filters.countries}
              showScore={$settings.bestPickMode}
              className="h-full w-full"
            />
          </div>
        {/if}

        <!-- top (interactive) -->
        <div
          class="absolute inset-0 z-20 origin-center touch-none"
          style={`transform: translate3d(${topX}px, ${topY}px, 0) rotate(${topRotate}deg);`}
          use:suppressTouchScroll
          onpointerdown={onPointerDown}
          onpointermove={onPointerMove}
          onpointerup={onPointerUp}
          onpointercancel={onPointerCancel}
          onkeydown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              flipped = !flipped;
              showAllSpellings = false;
            }
          }}
          role="button"
          tabindex="0"
        >
          <div class="h-full w-full [perspective:1200px]">
            <div
              class={`relative h-full w-full transition-transform duration-300 [transform-style:preserve-3d] ${
                flipped ? '[transform:rotateY(180deg)]' : ''
              }`}
            >
              <div class="absolute inset-0 [backface-visibility:hidden]">
                <NameCard
                  card={topCard}
                  showLastName={$settings.showLastName}
                  lastName={$settings.lastName}
                  selectedCountries={$filters.countries}
                  showScore={$settings.bestPickMode}
                  className="h-full w-full"
                />
              </div>

              <div
                class="absolute inset-0 overflow-hidden rounded-3xl border border-zinc-200 bg-white shadow-sm [backface-visibility:hidden] [transform:rotateY(180deg)]"
              >
                <div
                  class="pointer-events-none absolute inset-0"
                  style={`background: radial-gradient(circle at 30% 10%, rgba(${topTintRgb}, 0.12), transparent 60%);`}
                ></div>

                <div class="relative flex h-full flex-col p-5">
                  <div class="flex items-center justify-between gap-3">
                    <button
                      type="button"
                      class="inline-flex items-center gap-2 rounded-full border border-zinc-200 bg-white px-3 py-2 text-sm font-semibold text-zinc-700 shadow-sm hover:bg-zinc-50"
                      onclick={() => {
                        flipped = false;
                        showAllSpellings = false;
                      }}
                    >
                      <Icon name="chevronRight" className="rotate-180" />
                      Back
                    </button>
                    <div class="min-w-0 text-right">
                      <div class="truncate text-sm font-semibold text-zinc-900">Details</div>
                      <div class="truncate text-xs text-zinc-500">{topCard.display_name}</div>
                    </div>
                  </div>

                  <div class="mt-4 flex-1 space-y-3 overflow-auto pr-1">
                    <section class="rounded-2xl border border-zinc-200 bg-white/80 p-4">
                      <div class="flex items-center justify-between gap-3">
                        <div class="text-sm font-semibold text-zinc-900">Spellings</div>
                        <div class="flex items-center gap-3">
                          {#if detailsLoading && !topCard.all_spellings}
                            <Badge tone="neutral" label="Loading…" />
                          {/if}

                          {#if topCard.all_spellings && topCard.all_spellings.length > 6}
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

                      {#if showAllSpellings && topCard.all_spellings}
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
                        {#if detailsLoading && !topCard.all_spellings}
                          <div class="text-sm text-zinc-600">Fetching spellings…</div>
                        {:else}
                          {@const all = topCard.all_spellings ?? topCard.spellings_preview}
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
                      {#if topCard.within_concept_variants?.length}
                        <div class="mt-3 text-xs text-zinc-500">
                          Within-concept variants: {topCard.within_concept_variants.join(', ')}
                        </div>
                      {/if}
                    </section>

                    <section class="rounded-2xl border border-zinc-200 bg-white/80 p-4">
                      <div class="flex items-center justify-between gap-3">
                        <div class="text-sm font-semibold text-zinc-900">Popularity</div>
                        <Badge tone="neutral" label="per million births" />
                      </div>
                      <div class="mt-3 space-y-2">
                        {#each topCard.per_country as c (c.country)}
                          <div class="flex items-center justify-between text-sm">
                            <div class="font-medium text-zinc-800">{c.country}</div>
                            <div class="text-zinc-600">{Math.round(c.ppm)} ppm</div>
                          </div>
                        {/each}
                      </div>
                    </section>

                    <section class="rounded-2xl border border-zinc-200 bg-white/80 p-4">
                      <div class="text-sm font-semibold text-zinc-900">Sex split</div>
                      <div class="mt-3 space-y-4">
                        {#each $filters.countries as country (country)}
                          {@const split = splitFor(country)}
                          {@const total = split ? split.male + split.female : 0}
                          <div>
                            <div class="flex items-center justify-between text-xs text-zinc-500">
                              <div class="font-semibold">{countryLabel(country)}</div>
                              {#if split && total > 0}
                                <div>
                                  Male {Math.round(split.male_pct)}% · Female {Math.round(
                                    split.female_pct
                                  )}%
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
                      {#if detailsLoading && !topCard.last5_year_ranks}
                        <div class="mt-2 text-sm text-zinc-600">Fetching ranks…</div>
                      {:else if topCard.last5_years}
                        <div class="mt-3 space-y-4">
                          {#each $filters.countries as country (country)}
                            {@const years = rankYearsFor(country)}
                            {@const byYear = rankByYearFor(country)}
                            {#if years.length}
                              <div>
                                <div class="text-xs font-semibold text-zinc-500">
                                  {countryLabel(country)}
                                </div>
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

                  {#if topGenderInfo.note}
                    <div class="mt-3 text-center text-[11px] text-zinc-400">
                      {topGenderInfo.note}
                    </div>
                  {/if}
                </div>
              </div>
            </div>
          </div>

          {#if label.label}
            <div
              class={`pointer-events-none absolute left-6 top-6 rounded-2xl border px-3 py-1 text-sm font-bold tracking-wide ${
                label.tone === 'teal'
                  ? 'border-teal-300 bg-teal-50 text-teal-800'
                  : label.tone === 'rose'
                    ? 'border-rose-300 bg-rose-50 text-rose-800'
                    : 'border-amber-300 bg-amber-50 text-amber-900'
              }`}
              transition:fade={{ duration: 80 }}
            >
              {label.label}
            </div>
          {/if}
        </div>
      </div>
    </div>

    <div class="grid grid-cols-3 gap-3 pt-2">
      <button
        type="button"
        class="flex items-center justify-center rounded-full border border-rose-200 bg-rose-50 px-4 py-3 text-rose-800 shadow-sm hover:bg-rose-100 disabled:opacity-50"
        onclick={() => commitSwipeWithMomentum('nope', { startDx: 0, startDy: 0 })}
        disabled={!canSwipe()}
      >
        <span class="sr-only">Nope</span>
        <Icon name="x" size={24} />
      </button>
      <button
        type="button"
        class="flex items-center justify-center rounded-full border border-amber-200 bg-amber-50 px-4 py-3 text-amber-900 shadow-sm hover:bg-amber-100 disabled:opacity-50"
        onclick={() => commitSwipeWithMomentum('superlike', { startDx: 0, startDy: 0 })}
        disabled={!canSwipe()}
      >
        <span class="sr-only">Super-like</span>
        <Icon name="sparkle" size={24} />
      </button>
      <button
        type="button"
        class="flex items-center justify-center rounded-full border border-teal-200 bg-teal-50 px-4 py-3 text-teal-800 shadow-sm hover:bg-teal-100 disabled:opacity-50"
        onclick={() => commitSwipeWithMomentum('like', { startDx: 0, startDy: 0 })}
        disabled={!canSwipe()}
      >
        <span class="sr-only">Like</span>
        <Icon name="likes" size={24} />
      </button>
    </div>
  </div>
{/if}
