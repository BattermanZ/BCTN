<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import { currentUser, partnerUser } from '$lib/stores/app';

  let showHelp = $state(true);
</script>

<header class="flex items-end justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">Admin</h1>
    <div class="mt-1 text-sm text-zinc-600">Account management (mock UI).</div>
  </div>
  <Badge tone="indigo" label="Admin" />
</header>

{#if showHelp}
  <div class="mt-4 rounded-3xl border border-zinc-200 bg-white p-5 text-sm text-zinc-700">
    <div class="font-semibold">Note</div>
    <div class="mt-2">
      This screen is a mock. In the real app, it will be backed by the admin APIs and enforce the
      “max 2 accounts” rule.
    </div>
    <button
      type="button"
      class="mt-3 text-sm font-semibold text-indigo-600 hover:underline"
      onclick={() => (showHelp = false)}
    >
      Got it
    </button>
  </div>
{/if}

<div class="mt-4 space-y-4">
  <section class="rounded-3xl border border-zinc-200 bg-white p-5">
    <h2 class="text-sm font-semibold text-zinc-900">Users</h2>

    <div class="mt-3 space-y-3">
      <div class="rounded-2xl border border-zinc-200 bg-zinc-50 p-4">
        <div class="flex items-center justify-between">
          <div class="text-sm font-semibold">{$currentUser.username}</div>
          <Badge tone="indigo" label={$currentUser.is_admin ? 'Admin' : 'User'} />
        </div>
        <div class="mt-3 grid grid-cols-2 gap-2">
          <button
            type="button"
            class="rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm font-semibold text-zinc-700 hover:bg-zinc-50"
          >
            Edit
          </button>
          <button
            type="button"
            class="rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm font-semibold text-zinc-400"
            disabled
          >
            Delete
          </button>
        </div>
      </div>

      <div class="rounded-2xl border border-zinc-200 bg-zinc-50 p-4">
        <div class="flex items-center justify-between">
          <div class="text-sm font-semibold">{$partnerUser.username}</div>
          <Badge tone="neutral" label={$partnerUser.is_admin ? 'Admin' : 'User'} />
        </div>
        <div class="mt-3 grid grid-cols-2 gap-2">
          <button
            type="button"
            class="rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm font-semibold text-zinc-700 hover:bg-zinc-50"
          >
            Edit
          </button>
          <button
            type="button"
            class="rounded-2xl border border-rose-200 bg-rose-50 px-3 py-2 text-sm font-semibold text-rose-700 hover:bg-rose-100"
          >
            Delete
          </button>
        </div>
      </div>
    </div>

    <div class="mt-4 text-xs text-zinc-500">
      Create user is disabled in the mock because the product rule is exactly 2 accounts total.
    </div>
    <button
      type="button"
      class="mt-3 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm font-semibold text-zinc-400"
      disabled
    >
      Create new user
    </button>
  </section>

  <section class="rounded-3xl border border-zinc-200 bg-white p-5">
    <h2 class="text-sm font-semibold text-zinc-900">Reset password (mock)</h2>
    <div class="mt-3 grid gap-2">
      <input
        class="w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 shadow-sm"
        placeholder="New password"
        type="password"
      />
      <button
        type="button"
        class="rounded-2xl border border-indigo-200 bg-indigo-50 px-3 py-2 text-sm font-semibold text-indigo-800 hover:bg-indigo-100"
      >
        Save
      </button>
    </div>
  </section>
</div>
