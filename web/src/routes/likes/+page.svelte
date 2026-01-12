<script lang="ts">
  import BottomSheet from '$lib/components/BottomSheet.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import NameCard from '$lib/components/NameCard.svelte';
  import { api, type CardRef } from '$lib/api';
  import { filters, settings } from '$lib/stores/app';
  import type { Card } from '$lib/types';
  import { goto } from '$app/navigation';

  type Segment = 'likes' | 'superlikes';

  let segment = $state<Segment>('likes');
  let sheetOpen = $state(false);
  let refs = $state<CardRef[]>([]);
  let selected = $state<Card | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      refs = segment === 'likes' ? await api.likes() : await api.superlikes();
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
        return;
      }
      error = (e as Error | null)?.message ?? 'Failed to load likes';
    } finally {
      loading = false;
    }
  }

  async function open(ref: CardRef) {
    try {
      selected = await api.cardDetails(ref.card_key);
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

  $effect(() => {
    void segment;
    void load();
  });
</script>

<header class="flex items-end justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">Likes</h1>
    <div class="mt-1 text-sm text-zinc-600">Your likes & super-likes.</div>
  </div>
  <Badge tone={segment === 'likes' ? 'teal' : 'amber'} label={`${refs.length}`} />
</header>

<div class="mt-4">
  <div class="grid grid-cols-2 gap-2 rounded-2xl border border-zinc-200 bg-white p-1">
    <button
      type="button"
      class={`rounded-xl px-3 py-2 text-sm font-semibold ${
        segment === 'likes' ? 'bg-zinc-900 text-white' : 'text-zinc-600 hover:bg-zinc-50'
      }`}
      onclick={() => (segment = 'likes')}
    >
      Likes
    </button>
    <button
      type="button"
      class={`rounded-xl px-3 py-2 text-sm font-semibold ${
        segment === 'superlikes' ? 'bg-zinc-900 text-white' : 'text-zinc-600 hover:bg-zinc-50'
      }`}
      onclick={() => (segment = 'superlikes')}
    >
      Super-likes
    </button>
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
  {:else if refs.length === 0}
    <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-sm text-zinc-700">
      Nothing here yet.
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
          {#if segment === 'likes'}
            <Badge tone="teal" label="Liked" />
          {:else}
            <Badge tone="amber" label="Super" />
          {/if}
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
    <NameCard
      card={selected}
      showLastName={$settings.showLastName}
      lastName={$settings.lastName}
      selectedCountries={$filters.countries}
      showScore={$settings.bestPickMode}
    />
  {/if}
</BottomSheet>
