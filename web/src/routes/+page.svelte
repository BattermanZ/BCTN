<script lang="ts">
  import SwipeDeck from '$lib/components/SwipeDeck.svelte';
  import { mockCards } from '$lib/mock/cards';
</script>

<header class="flex items-center justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">BCTN</h1>
    <div class="mt-1 text-sm text-zinc-600">Swipe names. Match with your partner.</div>
  </div>
</header>

<div class="mt-4">
  <SwipeDeck cards={mockCards} />
</div>
