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
  let selected = $state<Card | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

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
      selected = await api.cardDetails(ref.card_key, {
        countries: $filters.countries.join(','),
        page: 0,
        page_size: 200,
      });
      sheetOpen = true;
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
      } else {
        error = (e as Error | null)?.message ?? 'Failed to load card';
      }
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
  <Badge tone="indigo" label={`${refs.length} matches`} />
</header>

<div class="mt-4 space-y-3">
  {#if error}
    <div class="rounded-3xl border border-rose-200 bg-rose-50 p-6 text-sm text-rose-800">
      {error}
    </div>
  {:else if loading}
    <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-sm text-zinc-700">
      Loading…
    </div>
  {:else if refs.length === 0}
    <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-sm text-zinc-700">
      No matches yet. Start swiping!
    </div>
  {:else}
    {#each refs as ref (ref.card_key)}
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
  {/if}
</BottomSheet>
