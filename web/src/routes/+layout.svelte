<script lang="ts">
  import '../app.css';
  import favicon from '$lib/assets/favicon.svg';
  import BottomTabs from '$lib/components/BottomTabs.svelte';
  import { api } from '$lib/api';
  import { currentUser, partnerUser, settings } from '$lib/stores/app';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';

  let { children } = $props();

  let authChecked = $state(false);

  async function bootstrap() {
    try {
      const me = await api.me();
      currentUser.set(me);

      if (me.must_change_password) {
        partnerUser.set(null);
        if ($page.url.pathname !== '/change-password') {
          await goto('/change-password');
        }
        return;
      }

      const users = await api.users();
      const partner = users.find((u) => u.id !== me.id) ?? null;
      partnerUser.set(partner);

      const s = await api.getSettings();
      settings.set({
        showLastName: s.show_last_name,
        lastName: s.last_name,
        bestPickMode: s.best_pick_mode,
      });

      if ($page.url.pathname === '/login') {
        await goto('/');
      }
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      currentUser.set(null);
      partnerUser.set(null);
      if (status === 401 && $page.url.pathname !== '/login') {
        await goto('/login');
      }
    } finally {
      authChecked = true;
    }
  }

  onMount(() => {
    void bootstrap();
  });
</script>

<svelte:head>
  <link rel="icon" href={favicon} />
  <meta name="theme-color" content="#ffffff" />
</svelte:head>

<div class="min-h-dvh bg-zinc-50 text-zinc-900">
  <main class="mx-auto max-w-md px-4 pb-28 pt-4">
    {@render children()}
  </main>
  {#if authChecked && $currentUser && !$currentUser.must_change_password && $page.url.pathname !== '/login' && $page.url.pathname !== '/change-password'}
    <BottomTabs />
  {/if}
</div>
