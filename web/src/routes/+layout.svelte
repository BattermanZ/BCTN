<script lang="ts">
  import '../app.css';
  import favicon from '$lib/assets/favicon.svg';
  import BottomTabs from '$lib/components/BottomTabs.svelte';
  import { api } from '$lib/api';
  import { parseCountriesCsv, parseGender, parsePopularityCsv } from '$lib/filter_parse';
  import { currentUser, filters, partnerUser, settings } from '$lib/stores/app';
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
      filters.set({
        countries: parseCountriesCsv(s.filter_countries),
        popularity: parsePopularityCsv(s.filter_popularity),
        gender: parseGender(s.filter_gender),
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
  <title>BCTN</title>
  <link rel="icon" href={favicon} />
  <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32.png" />
  <link rel="apple-touch-icon" href="/apple-touch-icon.png" />
  <link rel="manifest" href="/manifest.webmanifest" />
  <meta name="apple-mobile-web-app-capable" content="yes" />
  <meta name="apple-mobile-web-app-title" content="BCTN" />
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
