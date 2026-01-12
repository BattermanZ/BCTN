<script lang="ts">
  import SwipeDeck from '$lib/components/SwipeDeck.svelte';
  import { mockCards } from '$lib/mock/cards';
  import { filters, settings } from '$lib/stores/app';
</script>

<header class="flex items-center justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">BCTN</h1>
    <div class="mt-1 text-sm text-zinc-600">Swipe names. Match with your partner.</div>
  </div>
  <div class="flex items-center gap-2">
    <label class="flex cursor-pointer select-none items-center gap-2 text-sm text-zinc-600">
      <input
        type="checkbox"
        class="h-4 w-4 accent-indigo-600"
        checked={$settings.showLastName}
        onchange={(e) =>
          settings.update((s) => ({
            ...s,
            showLastName: (e.currentTarget as HTMLInputElement).checked,
          }))}
      />
      Last name
    </label>
  </div>
</header>

<div class="mt-4">
  <SwipeDeck cards={mockCards} />
</div>

<div class="mt-6 rounded-2xl border border-zinc-200 bg-white p-4 text-sm text-zinc-700">
  <div class="font-semibold">Current filters (mock)</div>
  <div class="mt-2 flex flex-wrap gap-2">
    <span class="rounded-full border border-zinc-200 bg-zinc-50 px-3 py-1">
      Countries: {$filters.countries.join(', ')}
    </span>
    <span class="rounded-full border border-zinc-200 bg-zinc-50 px-3 py-1">
      Popularity: {$filters.popularity}
    </span>
    <span class="rounded-full border border-zinc-200 bg-zinc-50 px-3 py-1">
      Gender: {$filters.gender}
    </span>
  </div>
</div>
