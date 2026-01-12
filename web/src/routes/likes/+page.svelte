<script lang="ts">
  import BottomSheet from '$lib/components/BottomSheet.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import NameCard from '$lib/components/NameCard.svelte';
  import { mockCards } from '$lib/mock/cards';
  import { type SwipeRecord } from '$lib/mock/swipeViews';
  import { filters, settings } from '$lib/stores/app';

  type Segment = 'likes' | 'superlikes';

  let segment = $state<Segment>('likes');
  let sheetOpen = $state(false);
  let selected = $state<(typeof mockCards)[number] | null>(null);

  const mockSwipes: SwipeRecord[] = [
    { card_key: 'amelie', action: 'like', by: 'you', at: 10 },
    { card_key: 'amelie', action: 'like', by: 'partner', at: 11 },
    { card_key: 'sacha', action: 'superlike', by: 'you', at: 12 },
    { card_key: 'sacha', action: 'like', by: 'partner', at: 18 },
    { card_key: 'aya', action: 'like', by: 'partner', at: 19 },
    { card_key: 'mila', action: 'superlike', by: 'partner', at: 15 },
  ];

  function open(card: (typeof mockCards)[number]) {
    selected = card;
    sheetOpen = true;
  }

  const likedKeys = $derived(
    mockSwipes
      .filter((s) => s.action === 'like')
      .map((s) => s.card_key)
      .filter((v, i, a) => a.indexOf(v) === i)
  );
  const superLikedKeys = $derived(
    mockSwipes
      .filter((s) => s.action === 'superlike')
      .map((s) => s.card_key)
      .filter((v, i, a) => a.indexOf(v) === i)
  );

  const visibleKeys = $derived(segment === 'likes' ? likedKeys : superLikedKeys);
  const list = $derived(mockCards.filter((c) => visibleKeys.includes(c.card_key)));
</script>

<header class="flex items-end justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">Likes</h1>
    <div class="mt-1 text-sm text-zinc-600">Shared likes & super-likes (mock data).</div>
  </div>
  <Badge tone={segment === 'likes' ? 'teal' : 'amber'} label={`${list.length}`} />
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
  {#if list.length === 0}
    <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-sm text-zinc-700">
      Nothing here yet.
    </div>
  {:else}
    {#each list as card (card.card_key)}
      <button
        type="button"
        class="w-full rounded-2xl border border-zinc-200 bg-white p-4 text-left shadow-sm hover:bg-zinc-50"
        onclick={() => open(card)}
      >
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0">
            <div class="truncate text-base font-semibold">{card.display_name}</div>
            <div class="mt-1 flex flex-wrap gap-2">
              {#each card.spellings_preview.slice(0, 2) as s (s)}
                <span
                  class="rounded-full border border-zinc-200 bg-zinc-50 px-3 py-1 text-xs text-zinc-700"
                >
                  {s}
                </span>
              {/each}
            </div>
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
