<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import { currentUser, filters, partnerUser, settings } from '$lib/stores/app';
  import type { CountryCode } from '$lib/types';
  import { resolveRoute } from '$app/paths';

  const countries: CountryCode[] = ['US', 'FR', 'CL'];

  function toggleCountry(c: CountryCode) {
    filters.update((f) => {
      const has = f.countries.includes(c);
      const next = has ? f.countries.filter((x) => x !== c) : [...f.countries, c];
      return { ...f, countries: next.length ? next : f.countries };
    });
  }
</script>

<header class="flex items-end justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">Settings</h1>
    <div class="mt-1 text-sm text-zinc-600 dark:text-zinc-300">Filters & display (mock).</div>
  </div>
  <Badge tone="neutral" label={$currentUser.username} />
</header>

<div class="mt-4 space-y-4">
  <section
    class="rounded-3xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900/40"
  >
    <h2 class="text-sm font-semibold text-zinc-900 dark:text-zinc-100">Filters</h2>

    <div class="mt-4">
      <div class="text-xs font-semibold text-zinc-500 dark:text-zinc-300">Countries</div>
      <div class="mt-2 flex gap-2">
        {#each countries as c (c)}
          <button
            type="button"
            class={`rounded-full border px-3 py-2 text-sm font-semibold ${
              $filters.countries.includes(c)
                ? 'border-indigo-200 bg-indigo-50 text-indigo-800 dark:border-indigo-900/60 dark:bg-indigo-950/40 dark:text-indigo-200'
                : 'border-zinc-200 bg-white text-zinc-700 hover:bg-zinc-50 dark:border-zinc-800 dark:bg-zinc-950 dark:text-zinc-200 dark:hover:bg-zinc-900'
            }`}
            onclick={() => toggleCountry(c)}
          >
            {c}
          </button>
        {/each}
      </div>
      <div class="mt-2 text-xs text-zinc-500 dark:text-zinc-300">
        Filters use “ANY selected country passes” semantics.
      </div>
    </div>

    <div class="mt-4">
      <div class="text-xs font-semibold text-zinc-500 dark:text-zinc-300">Popularity</div>
      <div class="mt-2 grid grid-cols-3 gap-2">
        {#each ['rare', 'common', 'top'] as p (p)}
          <button
            type="button"
            class={`rounded-2xl border px-3 py-2 text-sm font-semibold ${
              $filters.popularity === p
                ? 'border-zinc-900 bg-zinc-900 text-white dark:border-white dark:bg-white dark:text-zinc-900'
                : 'border-zinc-200 bg-white text-zinc-700 hover:bg-zinc-50 dark:border-zinc-800 dark:bg-zinc-950 dark:text-zinc-200 dark:hover:bg-zinc-900'
            }`}
            onclick={() => filters.update((f) => ({ ...f, popularity: p as typeof f.popularity }))}
          >
            {p}
          </button>
        {/each}
      </div>
      <label class="mt-3 flex items-center gap-2 text-sm text-zinc-700 dark:text-zinc-200">
        <input
          type="checkbox"
          class="h-4 w-4 accent-indigo-600"
          checked={$filters.includeExtraRare}
          onchange={(e) =>
            filters.update((f) => ({
              ...f,
              includeExtraRare: (e.currentTarget as HTMLInputElement).checked,
            }))}
        />
        Include extra rare
      </label>
    </div>

    <div class="mt-4">
      <div class="text-xs font-semibold text-zinc-500 dark:text-zinc-300">Gender</div>
      <select
        class="mt-2 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 shadow-sm dark:border-zinc-800 dark:bg-zinc-950 dark:text-zinc-200"
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

  <section
    class="rounded-3xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900/40"
  >
    <h2 class="text-sm font-semibold text-zinc-900 dark:text-zinc-100">Display</h2>

    <label class="mt-4 flex items-center justify-between gap-3">
      <div>
        <div class="text-sm font-semibold">Show last name</div>
        <div class="text-xs text-zinc-500 dark:text-zinc-300">
          Preview names with your last name on cards.
        </div>
      </div>
      <input
        type="checkbox"
        class="h-5 w-5 accent-indigo-600"
        checked={$settings.showLastName}
        onchange={(e) =>
          settings.update((s) => ({
            ...s,
            showLastName: (e.currentTarget as HTMLInputElement).checked,
          }))}
      />
    </label>

    <div class="mt-4">
      <div class="text-xs font-semibold text-zinc-500 dark:text-zinc-300">Last name</div>
      <input
        class="mt-2 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 shadow-sm dark:border-zinc-800 dark:bg-zinc-950 dark:text-zinc-200"
        value={$settings.lastName}
        oninput={(e) =>
          settings.update((s) => ({ ...s, lastName: (e.currentTarget as HTMLInputElement).value }))}
      />
    </div>

    <label class="mt-4 flex items-center justify-between gap-3">
      <div>
        <div class="text-sm font-semibold">Best pick mode</div>
        <div class="text-xs text-zinc-500 dark:text-zinc-300">
          Sorts the deck using the scoring system (off by default).
        </div>
      </div>
      <input
        type="checkbox"
        class="h-5 w-5 accent-indigo-600"
        checked={$settings.bestPickMode}
        onchange={(e) =>
          settings.update((s) => ({
            ...s,
            bestPickMode: (e.currentTarget as HTMLInputElement).checked,
          }))}
      />
    </label>
  </section>

  <section
    class="rounded-3xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900/40"
  >
    <h2 class="text-sm font-semibold text-zinc-900 dark:text-zinc-100">Accounts (mock)</h2>
    <div class="mt-3 space-y-2 text-sm text-zinc-700 dark:text-zinc-200">
      <div class="flex items-center justify-between">
        <div>{$currentUser.username}</div>
        <Badge tone="indigo" label={$currentUser.is_admin ? 'Admin' : 'User'} />
      </div>
      <div class="flex items-center justify-between">
        <div>{$partnerUser.username}</div>
        <Badge tone="neutral" label={$partnerUser.is_admin ? 'Admin' : 'User'} />
      </div>
      <a
        href={resolveRoute('/admin')}
        class="mt-2 inline-flex items-center gap-2 text-sm font-semibold text-indigo-600 hover:underline dark:text-indigo-400"
      >
        Open admin
      </a>
    </div>
  </section>
</div>
