<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import { api } from '$lib/api';
  import { currentUser, filters, partnerUser, settings } from '$lib/stores/app';
  import type { CountryCode } from '$lib/types';
  import { resolveRoute } from '$app/paths';
  import { goto } from '$app/navigation';

  const countries: CountryCode[] = ['US', 'FR', 'CL'];
  const countryLabel: Record<CountryCode, string> = {
    US: 'United States',
    FR: 'France',
    CL: 'Chile',
  };

  function toggleCountry(c: CountryCode) {
    filters.update((f) => {
      const has = f.countries.includes(c);
      const next = has ? f.countries.filter((x) => x !== c) : [...f.countries, c];
      return { ...f, countries: next.length ? next : f.countries };
    });
  }

  let lastNameTimer = $state<number | null>(null);

  async function patchSettings(patch: {
    show_last_name?: boolean;
    last_name?: string;
    best_pick_mode?: boolean;
  }) {
    try {
      const s = await api.patchSettings(patch);
      settings.set({
        showLastName: s.show_last_name,
        lastName: s.last_name,
        bestPickMode: s.best_pick_mode,
      });
    } catch (e) {
      const status = (e as { status?: number } | null)?.status;
      if (status === 401) {
        await goto('/login');
      }
    }
  }

  async function logout() {
    try {
      await api.logout();
    } finally {
      currentUser.set(null);
      partnerUser.set(null);
      await goto('/login');
    }
  }

  const popularityLevels = ['extra_rare', 'rare', 'common', 'top'] as const;
  const popularityLabel: Record<(typeof popularityLevels)[number], string> = {
    extra_rare: 'Extra rare',
    rare: 'Rare',
    common: 'Common',
    top: 'Top',
  };

  function togglePopularity(p: (typeof popularityLevels)[number]) {
    filters.update((f) => {
      const has = f.popularity.includes(p);
      const next = has ? f.popularity.filter((x) => x !== p) : [...f.popularity, p];
      return { ...f, popularity: next.length ? next : f.popularity };
    });
  }
</script>

<header class="flex items-end justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">Settings</h1>
    <div class="mt-1 text-sm text-zinc-600">Filters & display.</div>
  </div>
  {#if $currentUser}
    <Badge tone="neutral" label={$currentUser.username} />
  {/if}
</header>

<div class="mt-4 space-y-4">
  <section class="rounded-3xl border border-zinc-200 bg-white p-5">
    <h2 class="text-sm font-semibold text-zinc-900">Filters</h2>

    <div class="mt-4">
      <div class="text-xs font-semibold text-zinc-500">Countries</div>
      <div class="mt-2 flex gap-2">
        {#each countries as c (c)}
          <button
            type="button"
            class={`rounded-full border px-3 py-2 text-sm font-semibold ${
              $filters.countries.includes(c)
                ? 'border-indigo-200 bg-indigo-50 text-indigo-800'
                : 'border-zinc-200 bg-white text-zinc-700 hover:bg-zinc-50'
            }`}
            onclick={() => toggleCountry(c)}
          >
            {countryLabel[c]}
          </button>
        {/each}
      </div>
      <div class="mt-2 text-xs text-zinc-500">
        Filters use “ANY selected country passes” semantics.
      </div>
    </div>

    <div class="mt-4">
      <div class="text-xs font-semibold text-zinc-500">Popularity</div>
      <div class="mt-2 grid grid-cols-2 gap-2">
        {#each popularityLevels as p (p)}
          <button
            type="button"
            class={`rounded-2xl border px-3 py-2 text-sm font-semibold ${
              $filters.popularity.includes(p)
                ? 'border-zinc-900 bg-zinc-900 text-white'
                : 'border-zinc-200 bg-white text-zinc-700 hover:bg-zinc-50'
            }`}
            onclick={() => togglePopularity(p)}
          >
            {popularityLabel[p]}
          </button>
        {/each}
      </div>
      <div class="mt-2 text-xs text-zinc-500">
        Select one or more tiers. Default is Rare + Common + Top (Extra rare off).
      </div>
    </div>

    <div class="mt-4">
      <div class="text-xs font-semibold text-zinc-500">Gender</div>
      <select
        class="mt-2 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 shadow-sm"
        value={$filters.gender}
        onchange={(e) =>
          filters.update((f) => ({
            ...f,
            gender: (e.currentTarget as HTMLSelectElement).value as typeof f.gender,
          }))}
      >
        <option value="any">Any</option>
        <option value="unisex">Unisex</option>
        <option value="mostly_female">Mostly female</option>
        <option value="female">Female</option>
        <option value="mostly_male">Mostly male</option>
        <option value="male">Male</option>
      </select>
    </div>
  </section>

  <section class="rounded-3xl border border-zinc-200 bg-white p-5">
    <h2 class="text-sm font-semibold text-zinc-900">Display</h2>

    <label class="mt-4 flex items-center justify-between gap-3">
      <div>
        <div class="text-sm font-semibold">Show last name</div>
        <div class="text-xs text-zinc-500">Preview names with your last name on cards.</div>
      </div>
      <input
        type="checkbox"
        class="h-5 w-5 accent-indigo-600"
        checked={$settings.showLastName}
        onchange={(e) => {
          const checked = (e.currentTarget as HTMLInputElement).checked;
          settings.update((s) => ({ ...s, showLastName: checked }));
          void patchSettings({ show_last_name: checked });
        }}
      />
    </label>

    <div class="mt-4">
      <div class="text-xs font-semibold text-zinc-500">Last name</div>
      <input
        class="mt-2 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 shadow-sm"
        value={$settings.lastName}
        oninput={(e) => {
          const value = (e.currentTarget as HTMLInputElement).value;
          settings.update((s) => ({ ...s, lastName: value }));
          if (lastNameTimer !== null) window.clearTimeout(lastNameTimer);
          lastNameTimer = window.setTimeout(() => {
            void patchSettings({ last_name: value });
            lastNameTimer = null;
          }, 400);
        }}
      />
    </div>

    <label class="mt-4 flex items-center justify-between gap-3">
      <div>
        <div class="text-sm font-semibold">Best pick mode</div>
        <div class="text-xs text-zinc-500">
          Sorts the deck using the scoring system (off by default).
        </div>
      </div>
      <input
        type="checkbox"
        class="h-5 w-5 accent-indigo-600"
        checked={$settings.bestPickMode}
        onchange={(e) => {
          const checked = (e.currentTarget as HTMLInputElement).checked;
          settings.update((s) => ({ ...s, bestPickMode: checked }));
          void patchSettings({ best_pick_mode: checked });
        }}
      />
    </label>
  </section>

  <section class="rounded-3xl border border-zinc-200 bg-white p-5">
    <h2 class="text-sm font-semibold text-zinc-900">Accounts</h2>
    <div class="mt-3 space-y-2 text-sm text-zinc-700">
      {#if $currentUser}
        <div class="flex items-center justify-between">
          <div>{$currentUser.username}</div>
          <Badge tone="indigo" label={$currentUser.is_admin ? 'Admin' : 'User'} />
        </div>
      {/if}
      {#if $partnerUser}
        <div class="flex items-center justify-between">
          <div>{$partnerUser.username}</div>
          <Badge tone="neutral" label={$partnerUser.is_admin ? 'Admin' : 'User'} />
        </div>
      {/if}
      <a
        href={resolveRoute('/admin')}
        class="mt-2 inline-flex items-center gap-2 text-sm font-semibold text-indigo-600 hover:underline"
      >
        Open admin
      </a>

      <button
        type="button"
        class="mt-3 inline-flex w-full items-center justify-center rounded-2xl border border-zinc-200 bg-white px-4 py-3 text-sm font-semibold text-zinc-700 shadow-sm hover:bg-zinc-50"
        onclick={() => void logout()}
      >
        Log out
      </button>
    </div>
  </section>
</div>
