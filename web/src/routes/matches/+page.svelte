<script lang="ts">
  import BottomSheet from '$lib/components/BottomSheet.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import NameCard from '$lib/components/NameCard.svelte';
  import { mockCards } from '$lib/mock/cards';
  import { computeMatches, filterCardsByKeys, type SwipeRecord } from '$lib/mock/swipeViews';
  import { filters, settings } from '$lib/stores/app';

  let sheetOpen = $state(false);
  let selected = $state<(typeof mockCards)[number] | null>(null);

  const mockSwipes: SwipeRecord[] = [
    { card_key: 'amelie', action: 'like', by: 'you', at: 10 },
    { card_key: 'amelie', action: 'like', by: 'partner', at: 11 },
    { card_key: 'sacha', action: 'superlike', by: 'you', at: 12 },
    { card_key: 'sacha', action: 'like', by: 'partner', at: 18 },
    { card_key: 'noa', action: 'nope', by: 'you', at: 9 },
    { card_key: 'noa', action: 'like', by: 'partner', at: 13 },
  ];

  const matches = $derived(computeMatches(mockSwipes));
  const cards = $derived(
    filterCardsByKeys(
      mockCards,
      matches.map((m) => m.card_key)
    )
  );

  function open(card: (typeof mockCards)[number]) {
    selected = card;
    sheetOpen = true;
  }
</script>

<header class="flex items-end justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">Matches</h1>
    <div class="mt-1 text-sm text-zinc-600">Names you both liked (mock data).</div>
  </div>
  <Badge tone="indigo" label={`${cards.length} matches`} />
</header>

<div class="mt-4 space-y-3">
  {#if cards.length === 0}
    <div class="rounded-3xl border border-zinc-200 bg-white p-6 text-sm text-zinc-700">
      No matches yet. Start swiping!
    </div>
  {:else}
    {#each cards as card (card.card_key)}
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
    <NameCard
      card={selected}
      showLastName={$settings.showLastName}
      lastName={$settings.lastName}
      selectedCountries={$filters.countries}
      showScore={$settings.bestPickMode}
    />
  {/if}
</BottomSheet>
