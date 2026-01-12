<script lang="ts">
  import '../app.css';
  import favicon from '$lib/assets/favicon.svg';
  import BottomTabs from '$lib/components/BottomTabs.svelte';

  let { children } = $props();
</script>

<svelte:head>
  <link rel="icon" href={favicon} />
  <meta name="theme-color" content="#ffffff" />
</svelte:head>

<div class="min-h-dvh bg-zinc-50 text-zinc-900">
  <main class="mx-auto max-w-md px-4 pb-28 pt-4">
    {@render children()}
  </main>
  <BottomTabs />
</div>
