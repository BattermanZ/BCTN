<script lang="ts">
  import SwipeDeck from '$lib/components/SwipeDeck.svelte';
</script>

<header class="flex items-center justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">BCTN</h1>
    <div class="mt-1 truncate text-sm text-zinc-600">Binational Committee for the Twins Names</div>
    <div class="mt-1 text-sm text-zinc-600">Swipe names. Match with your partner.</div>
  </div>
</header>

<div class="mt-4">
  <SwipeDeck />
</div>
