<script lang="ts">
  import BottomSheet from '$lib/components/BottomSheet.svelte';
  import Icon from '$lib/components/Icon.svelte';
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
  let activePointerId = $state<number | null>(null);
  let startX = $state(0);
  let startY = $state(0);
  let dx = $state(0);
  let dy = $state(0);

  const remaining = $derived(cards.length - index);

  const topCard = $derived(cards[index]);
  const midCard = $derived(cards[index + 1]);
  const backCard = $derived(cards[index + 2]);

  type MoveSample = { t: number; dx: number; dy: number };
  let samples = $state<MoveSample[]>([]);

  let animating = $state(false);
  let animX = $state(0);
  let animY = $state(0);
  let rafId = $state<number | null>(null);
  let promoteT = $state(0);
  let promoteRafId = $state<number | null>(null);

  let history = $state<string[]>([]);

  const topX = $derived(dragging ? dx : animating ? animX : 0);
  const topY = $derived(dragging ? dy : animating ? animY : 0);
  const topRotate = $derived(topX / 18);

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
    dx = e.clientX - startX;
    dy = e.clientY - startY;

    const now = performance.now();
    samples = [...samples, { t: now, dx, dy }].filter((s) => now - s.t <= 120).slice(-6);
  }

  function onPointerUp(e: PointerEvent) {
    if (!dragging) return;
    if (activePointerId !== e.pointerId) return;

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
        openDetails();
      }
      resetDrag();
    }
  }

  function onPointerCancel(e: PointerEvent) {
    if (activePointerId !== e.pointerId) return;
    resetDrag();
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

    const cardKey = topCard.card_key;
    const { vx, vy } = sampleVelocity();

    // lock gesture immediately
    dragging = false;
    activePointerId = null;

    // record swipe immediately (even while animating)
    history = [...history, cardKey];
    swipes.update((s) => ({
      ...s,
      byCardKey: { ...s.byCardKey, [cardKey]: action },
      lastAction: { card_key: cardKey, action },
    }));

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
    index = Math.min(index + 1, cards.length);
    resetAnimation();
    // reset the stack back to its resting (stacked) state immediately after advancing
    if (promoteRafId !== null) {
      cancelAnimationFrame(promoteRafId);
      promoteRafId = null;
    }
    promoteT = 0;
  }

  function undo() {
    if (dragging || animating) return;
    const lastKey = history.at(-1);
    if (!lastKey) return;

    history = history.slice(0, -1);
    swipes.update((s) => {
      const nextBy = { ...s.byCardKey };
      delete nextBy[lastKey];
      return { ...s, byCardKey: nextBy, lastAction: undefined };
    });

    const pos = cards.findIndex((c) => c.card_key === lastKey);
    if (pos >= 0) {
      index = pos;
    } else if (index > 0) {
      index -= 1;
    }
  }

  function canSwipe(): boolean {
    return Boolean(topCard) && !dragging && !animating;
  }

  function openDetails() {
    if (!topCard) return;
    if (animating) return;
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

{#if !topCard}
  <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-center">
    <div class="text-lg font-semibold">No more names</div>
    <div class="mt-2 text-sm text-zinc-600">
      Try broadening filters (countries, gender, popularity) to keep exploring.
    </div>
  </div>
{:else}
  <div class="flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <div class="text-sm text-zinc-500">{remaining} left</div>
      <button
        type="button"
        class="rounded-full border border-zinc-200 bg-white px-3 py-1.5 text-sm font-medium text-zinc-700 hover:bg-zinc-50"
        onclick={undo}
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
          onpointerdown={onPointerDown}
          onpointermove={onPointerMove}
          onpointerup={onPointerUp}
          onpointercancel={onPointerCancel}
          onkeydown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              openDetails();
            }
          }}
          role="button"
          tabindex="0"
        >
          <NameCard
            card={topCard}
            showLastName={$settings.showLastName}
            lastName={$settings.lastName}
            selectedCountries={$filters.countries}
            showScore={$settings.bestPickMode}
            className="h-full w-full"
          />

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
        class="flex items-center justify-center rounded-full border border-teal-200 bg-teal-50 px-4 py-3 text-teal-800 shadow-sm hover:bg-teal-100 disabled:opacity-50"
        onclick={() => commitSwipeWithMomentum('like', { startDx: 0, startDy: 0 })}
        disabled={!canSwipe()}
      >
        <span class="sr-only">Like</span>
        <Icon name="likes" size={24} />
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
    </div>
  </div>
{/if}

<BottomSheet open={sheetOpen} title={topCard?.display_name} onClose={() => (sheetOpen = false)}>
  {#if topCard}
    <div class="space-y-5">
      <div class="rounded-2xl border border-zinc-200 bg-zinc-50 p-4">
        <div class="text-sm font-semibold">Spellings</div>
        <div class="mt-2 flex flex-wrap gap-2">
          {#each topCard.all_spellings?.slice(0, showAllSpellings ? 50 : 6) ?? topCard.spellings_preview as s (s)}
            <span
              class="rounded-full border border-zinc-200 bg-white px-3 py-1 text-sm text-zinc-700"
            >
              {s}
            </span>
          {/each}
        </div>
        {#if topCard.all_spellings && topCard.all_spellings.length > 6}
          <button
            type="button"
            class="mt-3 text-sm font-medium text-indigo-600 hover:underline"
            onclick={() => (showAllSpellings = !showAllSpellings)}
          >
            {showAllSpellings ? 'Show fewer' : 'See all spellings'}
          </button>
        {/if}

        {#if topCard.within_concept_variants?.length}
          <div class="mt-4 text-xs text-zinc-500">
            Within-concept variants: {topCard.within_concept_variants.join(', ')}
          </div>
        {/if}
      </div>

      <div class="rounded-2xl border border-zinc-200 bg-zinc-50 p-4">
        <div class="text-sm font-semibold">Popularity (per million births)</div>
        <div class="mt-3 space-y-2">
          {#each topCard.per_country as c (c.country)}
            <div class="flex items-center justify-between text-sm">
              <div class="font-medium">{c.country}</div>
              <div class="text-zinc-600">{Math.round(c.ppm)} ppm</div>
            </div>
          {/each}
        </div>
      </div>

      <div class="rounded-2xl border border-zinc-200 bg-zinc-50 p-4">
        <div class="text-sm font-semibold">Rank (last 5 years)</div>
        {#if topCard.last5_year_ranks}
          <div class="mt-3 space-y-4">
            {#each rankEntries(topCard.last5_year_ranks) as [country, ranks] (country)}
              <div>
                <div class="text-xs font-semibold text-zinc-500">{country}</div>
                <div class="mt-2 grid grid-cols-5 gap-2 text-center text-xs">
                  {#each ranks as r (r.year)}
                    <div class="rounded-xl border border-zinc-200 bg-white p-2">
                      <div class="text-zinc-500">{r.year}</div>
                      <div class="mt-1 font-semibold">#{r.rank}</div>
                    </div>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        {:else}
          <div class="mt-2 text-sm text-zinc-600">Not available in mock data.</div>
        {/if}
      </div>
    </div>
  {/if}
</BottomSheet>
