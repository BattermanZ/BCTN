<script lang="ts">
  import { api } from '$lib/api';
  import { currentUser, partnerUser, settings } from '$lib/stores/app';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';

  let newPassword = $state('');
  let confirmPassword = $state('');
  let submitting = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    const me = await api.me().catch(() => null);
    if (!me) {
      await goto('/login');
      return;
    }
    currentUser.set(me);
    if (!me.must_change_password) {
      await goto('/');
    }
  });

  async function submit() {
    error = null;
    if (newPassword.length < 8) {
      error = 'Password must be at least 8 characters.';
      return;
    }
    if (newPassword !== confirmPassword) {
      error = 'Passwords do not match.';
      return;
    }

    submitting = true;
    try {
      await api.changePassword(newPassword);

      const me = await api.me();
      currentUser.set(me);

      const users = await api.users();
      const partner = users.find((u) => u.id !== me.id) ?? null;
      partnerUser.set(partner);

      const s = await api.getSettings();
      settings.set({
        showLastName: s.show_last_name,
        lastName: s.last_name,
        bestPickMode: s.best_pick_mode,
      });

      await goto('/');
    } catch (e) {
      const msg = (e as Error | null)?.message ?? 'Could not change password';
      error = msg || 'Could not change password';
    } finally {
      submitting = false;
    }
  }
</script>

<div class="mx-auto mt-8 max-w-sm">
  <h1 class="text-2xl font-semibold tracking-tight text-zinc-900">Set a new password</h1>
  <div class="mt-2 text-sm text-zinc-600">
    For security, you must change your password before using the app.
  </div>

  <form
    class="mt-6 space-y-3 rounded-3xl border border-zinc-200 bg-white p-5 shadow-sm"
    onsubmit={(e) => {
      e.preventDefault();
      void submit();
    }}
  >
    <label class="block">
      <div class="text-xs font-semibold text-zinc-500">New password</div>
      <input
        class="mt-2 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-base text-zinc-800 shadow-sm"
        type="password"
        autocomplete="new-password"
        value={newPassword}
        oninput={(e) => (newPassword = (e.currentTarget as HTMLInputElement).value)}
      />
    </label>

    <label class="block">
      <div class="text-xs font-semibold text-zinc-500">Confirm password</div>
      <input
        class="mt-2 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-base text-zinc-800 shadow-sm"
        type="password"
        autocomplete="new-password"
        value={confirmPassword}
        oninput={(e) => (confirmPassword = (e.currentTarget as HTMLInputElement).value)}
      />
    </label>

    {#if error}
      <div class="rounded-2xl border border-rose-200 bg-rose-50 p-3 text-sm text-rose-800">
        {error}
      </div>
    {/if}

    <button
      type="submit"
      class="mt-2 w-full rounded-2xl bg-zinc-900 px-4 py-3 text-sm font-semibold text-white hover:bg-zinc-800 disabled:opacity-50"
      disabled={submitting || !newPassword || !confirmPassword}
    >
      {submitting ? 'Saving…' : 'Save password'}
    </button>
  </form>
</div>
