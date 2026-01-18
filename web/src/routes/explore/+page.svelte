<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import BottomSheet from '$lib/components/BottomSheet.svelte';
  import FullCard from '$lib/components/FullCard.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import { api, type ExploreCard } from '$lib/api';
  import { goto } from '$app/navigation';
  import type { Card, CountryCode, SwipeAction } from '$lib/types';
  import { settings } from '$lib/stores/app';

  type Mode = 'search' | 'top';
  type Gender = 'any' | 'male' | 'female';

  let mode = $state<Mode>('top');

  let selectedCountries = $state<CountryCode[]>(['US', 'FR', 'CL']);
  let gender = $state<Gender>('any');

  let filtersOpen = $state(true);

  // Search
  let q = $state('');
  let reloadTimer: number | null = null;

  // Data
  let items = $state<ExploreCard[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let page = $state(0);
  const pageSize = 50;
  let hasMore = $state(true);

  // Sheet
  let sheetOpen = $state(false);
  let selected = $state<Card | null>(null);
  let selectedRef = $state<ExploreCard | null>(null);
  let sheetError = $state<string | null>(null);

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

  function toggleCountry(c: CountryCode) {
    if (selectedCountries.includes(c)) {
      if (selectedCountries.length === 1) return;
      selectedCountries = selectedCountries.filter((x) => x !== c);
    } else {
      selectedCountries = [...selectedCountries, c];
    }
  }

  function maxPpmLabel(card: ExploreCard): string {
    const candidates = card.per_country.filter((c) =>
      selectedCountries.includes(c.country as CountryCode)
    );
    if (!candidates.length) return '—';
    const best = candidates.reduce((a, b) => (b.ppm > a.ppm ? b : a));
    const ppm = Math.round(best.ppm);
    return `${best.country} ${ppm.toLocaleString()} ppm`;
  }

  function clearResults() {
    items = [];
    page = 0;
    hasMore = true;
    error = null;
  }

  async function loadPage(nextPage: number) {
    if (loading) return;
    if (!hasMore && nextPage !== 0) return;

    loading = true;
    error = null;
    try {
      const countries = selectedCountries.join(',');
      if (mode === 'search') {
        const query = q.trim();
        if (query.length < 2) {
          items = [];
          hasMore = false;
          return;
        }
        const res = await api.exploreSearch({
          q: query,
          countries,
          gender,
          page: nextPage,
          page_size: pageSize,
        });
        if (nextPage === 0) items = res;
        else items = [...items, ...res];
        hasMore = res.length === pageSize;
        page = nextPage;
      } else {
        const res = await api.exploreTop({
          countries,
          gender,
          page: nextPage,
          page_size: pageSize,
        });
        if (nextPage === 0) items = res;
        else items = [...items, ...res];
        hasMore = res.length === pageSize;
        page = nextPage;
      }
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
        return;
      }
      error = (e as Error | null)?.message ?? 'Failed to load';
      hasMore = false;
    } finally {
      loading = false;
    }
  }

  function scheduleSearch() {
    if (reloadTimer) window.clearTimeout(reloadTimer);
    const query = q.trim();
    clearResults();
    if (query.length < 2) {
      items = [];
      hasMore = false;
      return;
    }
    reloadTimer = window.setTimeout(() => void loadPage(0), 250);
  }

  $effect(() => {
    void selectedCountries;
    void gender;
    void mode;
    void q;

    if (reloadTimer) window.clearTimeout(reloadTimer);
    if (mode === 'search') {
      scheduleSearch();
      return;
    }

    clearResults();
    queueMicrotask(() => void loadPage(0));
  });

  async function open(ref: ExploreCard) {
    sheetError = null;
    try {
      selectedRef = ref;
      selected = await api.cardDetails(ref.card_key, {
        countries: selectedCountries.join(','),
        page: 0,
        page_size: 200,
      });
      sheetOpen = true;
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
      } else {
        sheetError = (e as Error | null)?.message ?? 'Failed to load card';
      }
    }
  }

  function closeSheet() {
    sheetOpen = false;
    selected = null;
    selectedRef = null;
    sheetError = null;
  }

  async function setAction(action: SwipeAction) {
    if (!selectedRef) return;
    if (selectedRef.my_action === action) {
      closeSheet();
      return;
    }
    try {
      const createRes = await api.createSwipe({
        card_key: selectedRef.card_key,
        card_kind: selectedRef.card_kind,
        action,
      });
      if (createRes.status === 'already_swiped') {
        await api.updateSwipe({ card_key: selectedRef.card_key, action });
      }
      selectedRef = { ...selectedRef, my_action: action };
      items = items.map((x) =>
        x.card_key === selectedRef!.card_key ? { ...x, my_action: action } : x
      );
      closeSheet();
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
        return;
      }
      sheetError = (e as Error | null)?.message ?? 'Failed to record swipe';
    }
  }
</script>

<header class="flex items-end justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">Explore</h1>
    <div class="mt-1 text-sm text-zinc-600">Search or browse top names.</div>
  </div>
  <Badge tone="neutral" label={`${items.length}`} />
</header>

<div class="mt-4 grid grid-cols-2 gap-2 rounded-2xl border border-zinc-200 bg-white p-1">
  <button
    type="button"
    class={`rounded-xl px-3 py-2 text-sm font-semibold ${
      mode === 'search' ? 'bg-zinc-900 text-white' : 'text-zinc-600 hover:bg-zinc-50'
    }`}
    onclick={() => (mode = 'search')}
  >
    Search
  </button>
  <button
    type="button"
    class={`rounded-xl px-3 py-2 text-sm font-semibold ${
      mode === 'top' ? 'bg-zinc-900 text-white' : 'text-zinc-600 hover:bg-zinc-50'
    }`}
    onclick={() => (mode = 'top')}
  >
    Top
  </button>
</div>

<section class="mt-4 rounded-3xl border border-zinc-200 bg-white p-4">
  <button
    type="button"
    class="flex w-full items-center justify-between gap-3"
    onclick={() => (filtersOpen = !filtersOpen)}
  >
    <div class="text-sm font-semibold text-zinc-900">Filters</div>
    <div class="text-xs font-semibold text-zinc-600">
      {filtersOpen ? 'Hide' : 'Show'}
    </div>
  </button>

  {#if filtersOpen}
    <div class="mt-4 space-y-4">
      <div>
        <div class="text-xs font-semibold text-zinc-500">Countries</div>
        <div class="mt-2 flex flex-wrap gap-2">
          {#each (['US', 'FR', 'CL'] as CountryCode[]) as c (c)}
            <button
              type="button"
              class={`rounded-full border px-3 py-2 text-sm font-semibold ${
                selectedCountries.includes(c)
                  ? 'border-indigo-300 bg-indigo-50 text-indigo-700'
                  : 'border-zinc-200 bg-white text-zinc-700 hover:bg-zinc-50'
              }`}
              onclick={() => toggleCountry(c)}
            >
              {countryLabel(c)}
            </button>
          {/each}
        </div>
      </div>

      <div>
        <div class="text-xs font-semibold text-zinc-500">Gender</div>
        <select
          class="mt-2 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-3 text-sm font-semibold text-zinc-800"
          bind:value={gender}
        >
          <option value="any">Any</option>
          <option value="male">Male</option>
          <option value="female">Female</option>
        </select>
        <div class="mt-2 text-xs text-zinc-500">
          Filters use “ANY selected country passes” semantics.
        </div>
      </div>

      {#if mode === 'search'}
        <div class="text-xs text-zinc-500">Fuzzy search enabled (matches similar spellings).</div>
      {:else}
        <div class="text-xs text-zinc-500">Sorted by births per million (ppm).</div>
      {/if}
    </div>
  {/if}
</section>

{#if mode === 'search'}
  <div class="mt-4 rounded-3xl border border-zinc-200 bg-white p-4">
    <div class="flex items-center gap-2">
      <input
        class="w-full rounded-2xl border border-zinc-200 bg-white px-3 py-3 text-sm font-semibold text-zinc-900 placeholder:text-zinc-400"
        placeholder="Search names…"
        bind:value={q}
      />
      {#if q.trim().length}
        <button
          type="button"
          class="rounded-2xl border border-zinc-200 bg-white p-3 text-zinc-700 hover:bg-zinc-50"
          onclick={() => (q = '')}
          aria-label="Clear"
        >
          <Icon name="x" />
        </button>
      {/if}
    </div>
    <div class="mt-2 text-xs text-zinc-500">Tip: try “amelie”, “mateo”, “zoe”…</div>
  </div>
{/if}

<div class="mt-4 space-y-3">
  {#if error}
    <div class="rounded-3xl border border-rose-200 bg-rose-50 p-6 text-sm text-rose-800">
      {error}
    </div>
  {:else if loading && items.length === 0}
    <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-sm text-zinc-700">
      Loading…
    </div>
  {:else if items.length === 0}
    <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-sm text-zinc-700">
      {#if mode === 'search' && q.trim().length < 2}
        Type at least 2 characters to search.
      {:else}
        No results. Try broader filters.
      {/if}
    </div>
  {:else}
    {#each items as item, i (item.card_key)}
      <button
        type="button"
        class="w-full rounded-2xl border border-zinc-200 bg-white p-4 text-left shadow-sm hover:bg-zinc-50"
        onclick={() => open(item)}
      >
        <div class="flex items-start justify-between gap-3">
          <div class="min-w-0">
            <div class="truncate text-base font-semibold">{item.display_name}</div>
            <div class="mt-1 text-xs text-zinc-500">
              {#if mode === 'top'}
                #{page * pageSize + i + 1} · {maxPpmLabel(item)}
              {:else}
                {maxPpmLabel(item)}
              {/if}
            </div>
          </div>
          <Badge tone="neutral" label={item.card_kind === 'group' ? 'Group' : 'Concept'} />
        </div>
      </button>
    {/each}

    {#if hasMore}
      <button
        type="button"
        class="w-full rounded-2xl border border-zinc-200 bg-white p-4 text-sm font-semibold text-zinc-700 hover:bg-zinc-50 disabled:opacity-50"
        onclick={() => loadPage(page + 1)}
        disabled={loading}
      >
        {loading ? 'Loading…' : 'Load more'}
      </button>
    {/if}
  {/if}
</div>

<BottomSheet open={sheetOpen} title={selected?.display_name} onClose={closeSheet}>
  {#if sheetError}
    <div class="rounded-3xl border border-rose-200 bg-rose-50 p-6 text-sm text-rose-800">
      {sheetError}
    </div>
  {:else if selected}
    <FullCard
      initialCard={selected}
      showLastName={$settings.showLastName}
      lastName={$settings.lastName}
      selectedCountries={selectedCountries}
      showScore={$settings.bestPickMode}
    />

    {#if selectedRef}
      <div class="mt-4 rounded-2xl border border-zinc-200 bg-white p-3">
        <div class="flex items-center justify-between gap-2">
          <button
            type="button"
            class="rounded-xl border border-rose-200 bg-rose-50 px-3 py-2 text-sm font-semibold text-rose-700 hover:bg-rose-100"
            onclick={() => setAction('nope')}
          >
            Remove
          </button>

          <div class="flex items-center gap-2">
            <button
              type="button"
              class="rounded-xl bg-zinc-900 px-3 py-2 text-sm font-semibold text-white hover:bg-zinc-800"
              onclick={() => setAction('like')}
            >
              Like
            </button>
            <button
              type="button"
              class="rounded-xl border border-amber-200 bg-amber-50 px-3 py-2 text-sm font-semibold text-amber-800 hover:bg-amber-100"
              onclick={() => setAction('superlike')}
            >
              Super-like
            </button>
          </div>
        </div>
        <div class="mt-2 text-xs text-zinc-500">
          Actions here count as swipes and remove the name from your deck.
        </div>
      </div>
    {/if}
  {/if}
</BottomSheet>
