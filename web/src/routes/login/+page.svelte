<script lang="ts">
  import { api } from '$lib/api';
  import { currentUser, partnerUser, settings } from '$lib/stores/app';
  import { goto } from '$app/navigation';

  let username = $state('');
  let password = $state('');
  let submitting = $state(false);
  let error = $state<string | null>(null);

  async function submit() {
    error = null;
    submitting = true;
    try {
      const me = await api.login(username.trim(), password);
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

      // Prevent iOS Safari "input focus zoom" from persisting after navigation.
      (document.activeElement as HTMLElement | null)?.blur?.();
      await goto('/');
    } catch (e) {
      const msg = (e as Error | null)?.message ?? 'Login failed';
      error = msg || 'Login failed';
    } finally {
      submitting = false;
    }
  }
</script>

<div class="mx-auto mt-8 max-w-sm">
  <h1 class="text-2xl font-semibold tracking-tight text-zinc-900">Sign in</h1>
  <div class="mt-2 text-sm text-zinc-600">BCTN is private — use your account credentials.</div>

  <form
    class="mt-6 space-y-3 rounded-3xl border border-zinc-200 bg-white p-5 shadow-sm"
    onsubmit={(e) => {
      e.preventDefault();
      void submit();
    }}
  >
    <label class="block">
      <div class="text-xs font-semibold text-zinc-500">Username</div>
      <input
        class="mt-2 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-base text-zinc-800 shadow-sm"
        autocomplete="username"
        value={username}
        oninput={(e) => (username = (e.currentTarget as HTMLInputElement).value)}
      />
    </label>

    <label class="block">
      <div class="text-xs font-semibold text-zinc-500">Password</div>
      <input
        class="mt-2 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-base text-zinc-800 shadow-sm"
        type="password"
        autocomplete="current-password"
        value={password}
        oninput={(e) => (password = (e.currentTarget as HTMLInputElement).value)}
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
      disabled={submitting || !username.trim() || !password}
    >
      {submitting ? 'Signing in…' : 'Sign in'}
    </button>
  </form>
</div>
