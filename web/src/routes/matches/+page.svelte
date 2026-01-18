<script lang="ts">
  import BottomSheet from '$lib/components/BottomSheet.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import FullCard from '$lib/components/FullCard.svelte';
  import { api, type CardRef } from '$lib/api';
  import { filters, settings } from '$lib/stores/app';
  import type { Card } from '$lib/types';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  let sheetOpen = $state(false);
  let refs = $state<CardRef[]>([]);
  let filterMale = $state(false);
  let filterFemale = $state(false);
  let selectedRef = $state<CardRef | null>(null);
  let selected = $state<Card | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  function matchesGender(ref: CardRef): boolean {
    if (filterMale === filterFemale) return true; // both or neither => Any
    if (filterMale) {
      return ref.gender === 'male' || ref.gender === 'mostly_male' || ref.gender === 'unisex';
    }
    return ref.gender === 'female' || ref.gender === 'mostly_female' || ref.gender === 'unisex';
  }

  const shownRefs = $derived(refs.filter(matchesGender));

  async function load() {
    loading = true;
    error = null;
    try {
      refs = await api.matches();
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
        return;
      }
      error = (e as Error | null)?.message ?? 'Failed to load matches';
    } finally {
      loading = false;
    }
  }

  async function open(ref: CardRef) {
    try {
      selectedRef = ref;
      selected = await api.cardDetails(ref.card_key, {
        countries: $filters.countries.join(','),
        page: 0,
        page_size: 200,
      });
      sheetOpen = true;
    } catch (e) {
      selectedRef = null;
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
      } else {
        error = (e as Error | null)?.message ?? 'Failed to load card';
      }
    }
  }

  async function update(action: 'nope' | 'like' | 'superlike') {
    if (!selectedRef) return;
    try {
      await api.updateSwipe({ card_key: selectedRef.card_key, action });
      if (action === 'nope') {
        sheetOpen = false;
        selected = null;
        selectedRef = null;
        await load();
        return;
      }

      selectedRef = { ...selectedRef, my_action: action };
      await load();
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
        return;
      }
      error = (e as Error | null)?.message ?? 'Failed to update swipe';
    }
  }

  onMount(() => {
    void load();
  });
</script>

<header class="flex items-end justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">Matches</h1>
    <div class="mt-1 text-sm text-zinc-600">Names you both liked.</div>
  </div>
  <Badge tone="indigo" label={`${shownRefs.length} matches`} />
</header>

<div class="mt-4">
  <div class="flex flex-wrap items-center gap-2">
    <button
      type="button"
      class={`rounded-full border px-3 py-2 text-sm font-semibold ${
        filterMale
          ? 'border-indigo-300 bg-indigo-50 text-indigo-700'
          : 'border-zinc-200 bg-white text-zinc-700 hover:bg-zinc-50'
      }`}
      onclick={() => (filterMale = !filterMale)}
    >
      Male
    </button>
    <button
      type="button"
      class={`rounded-full border px-3 py-2 text-sm font-semibold ${
        filterFemale
          ? 'border-indigo-300 bg-indigo-50 text-indigo-700'
          : 'border-zinc-200 bg-white text-zinc-700 hover:bg-zinc-50'
      }`}
      onclick={() => (filterFemale = !filterFemale)}
    >
      Female
    </button>
    <div class="text-xs text-zinc-500">Unisex appears in both.</div>
  </div>
</div>

<div class="mt-4 space-y-3">
  {#if error}
    <div class="rounded-3xl border border-rose-200 bg-rose-50 p-6 text-sm text-rose-800">
      {error}
    </div>
  {:else if loading}
    <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-sm text-zinc-700">
      Loading…
    </div>
  {:else if shownRefs.length === 0}
    <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-sm text-zinc-700">
      No matches for this filter.
    </div>
  {:else}
    {#each shownRefs as ref (ref.card_key)}
      <button
        type="button"
        class="w-full rounded-2xl border border-zinc-200 bg-white p-4 text-left shadow-sm hover:bg-zinc-50"
        onclick={() => open(ref)}
      >
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0">
            <div class="truncate text-base font-semibold">{ref.display_name}</div>
          </div>
          <Badge tone="teal" label="Matched" />
        </div>
      </button>
    {/each}
  {/if}
</div>

<BottomSheet
  open={sheetOpen}
  title={selected?.display_name}
  onClose={() => {
    sheetOpen = false;
    selected = null;
    selectedRef = null;
  }}
>
  {#if selected}
    <FullCard
      initialCard={selected}
      showLastName={$settings.showLastName}
      lastName={$settings.lastName}
      selectedCountries={$filters.countries}
      showScore={$settings.bestPickMode}
    />

    {#if selectedRef}
      <div class="mt-4 rounded-2xl border border-zinc-200 bg-white p-3">
        <div class="flex items-center justify-between gap-2">
          <button
            type="button"
            class="rounded-xl border border-rose-200 bg-rose-50 px-3 py-2 text-sm font-semibold text-rose-700 hover:bg-rose-100"
            onclick={() => update('nope')}
          >
            Remove
          </button>

          <div class="flex items-center gap-2">
            <button
              type="button"
              class={`rounded-xl px-3 py-2 text-sm font-semibold ${
                selectedRef.my_action === 'like'
                  ? 'bg-zinc-900 text-white'
                  : 'border border-zinc-200 bg-white text-zinc-700 hover:bg-zinc-50'
              }`}
              onclick={() => update('like')}
            >
              Like
            </button>
            <button
              type="button"
              class={`rounded-xl px-3 py-2 text-sm font-semibold ${
                selectedRef.my_action === 'superlike'
                  ? 'bg-zinc-900 text-white'
                  : 'border border-zinc-200 bg-white text-zinc-700 hover:bg-zinc-50'
              }`}
              onclick={() => update('superlike')}
            >
              Super-like
            </button>
          </div>
        </div>
        <div class="mt-2 text-xs text-zinc-500">
          Removing your swipe clears the match for both users and won’t show the name again.
        </div>
      </div>
    {/if}
  {/if}
</BottomSheet>
