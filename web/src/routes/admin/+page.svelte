<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import { api, type AdminUserRow } from '$lib/api';
  import { currentUser, partnerUser } from '$lib/stores/app';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';

  let loading = $state(true);
  let error = $state<string | null>(null);

  let users = $state<AdminUserRow[]>([]);

  let createUsername = $state('');
  let createPassword = $state('');
  let createIsAdmin = $state(false);
  let creating = $state(false);

  let editingId = $state<number | null>(null);
  let editUsername = $state('');
  let editPassword = $state('');
  let editIsAdmin = $state(false);
  let saving = $state(false);

  async function refreshUsers() {
    users = await api.adminUsers();
  }

  onMount(async () => {
    loading = true;
    error = null;
    try {
      let me = get(currentUser);
      if (!me) {
        me = await api.me().catch(() => null);
        if (me) currentUser.set(me);
      }
      if (!me) {
        await goto('/login');
        return;
      }
      if (!me.is_admin) {
        return;
      }
      await refreshUsers();
    } catch (e) {
      const msg = (e as Error | null)?.message ?? 'Could not load users';
      error = msg || 'Could not load users';
    } finally {
      loading = false;
    }
  });

  function startEdit(u: AdminUserRow) {
    editingId = u.id;
    editUsername = u.username;
    editPassword = '';
    editIsAdmin = u.is_admin;
  }

  function cancelEdit() {
    editingId = null;
    editUsername = '';
    editPassword = '';
    editIsAdmin = false;
  }

  async function saveEdit() {
    if (editingId === null) return;
    error = null;
    saving = true;
    try {
      const patch: { username?: string; password?: string; is_admin?: boolean } = {};
      if (editUsername.trim()) patch.username = editUsername.trim();
      patch.is_admin = editIsAdmin;
      if (editPassword) patch.password = editPassword;
      await api.adminUpdateUser(editingId, patch);
      await refreshUsers();

      const me = get(currentUser);
      const all = await api.users();
      const partner = me ? (all.find((u) => u.id !== me.id) ?? null) : null;
      partnerUser.set(partner);
      cancelEdit();
    } catch (e) {
      const msg = (e as Error | null)?.message ?? 'Could not save user';
      error = msg || 'Could not save user';
    } finally {
      saving = false;
    }
  }

  async function deleteUser(u: AdminUserRow) {
    if (!confirm(`Delete ${u.username}? This also deletes all swipes & matches for that user.`))
      return;
    error = null;
    try {
      await api.adminDeleteUser(u.id);
      await refreshUsers();

      const me = get(currentUser);
      const all = await api.users();
      const partner = me ? (all.find((p) => p.id !== me.id) ?? null) : null;
      partnerUser.set(partner);
    } catch (e) {
      const msg = (e as Error | null)?.message ?? 'Could not delete user';
      error = msg || 'Could not delete user';
    }
  }

  async function createUser() {
    error = null;
    if (!createUsername.trim()) {
      error = 'Username required.';
      return;
    }
    if (createPassword.length < 8) {
      error = 'Password must be at least 8 characters.';
      return;
    }
    creating = true;
    try {
      await api.adminCreateUser({
        username: createUsername.trim(),
        password: createPassword,
        is_admin: createIsAdmin,
      });
      createUsername = '';
      createPassword = '';
      createIsAdmin = false;
      await refreshUsers();

      const me = get(currentUser);
      const all = await api.users();
      const partner = me ? (all.find((u) => u.id !== me.id) ?? null) : null;
      partnerUser.set(partner);
    } catch (e) {
      const msg = (e as Error | null)?.message ?? 'Could not create user';
      error = msg || 'Could not create user';
    } finally {
      creating = false;
    }
  }
</script>

<header class="flex items-end justify-between">
  <div>
    <h1 class="text-xl font-semibold tracking-tight">Admin</h1>
    <div class="mt-1 text-sm text-zinc-600">Account management.</div>
  </div>
  <Badge tone="indigo" label="Admin" />
</header>

{#if !$currentUser}
  <div class="mt-4 rounded-3xl border border-zinc-200 bg-white p-5 text-sm text-zinc-700">
    Loading…
  </div>
{:else if !$currentUser.is_admin}
  <div class="mt-4 rounded-3xl border border-rose-200 bg-rose-50 p-5 text-sm text-rose-800">
    Admin access required.
  </div>
{:else}
  {#if error}
    <div class="mt-4 rounded-3xl border border-rose-200 bg-rose-50 p-5 text-sm text-rose-800">
      {error}
    </div>
  {/if}

  <div class="mt-4 space-y-4">
    <section class="rounded-3xl border border-zinc-200 bg-white p-5">
      <h2 class="text-sm font-semibold text-zinc-900">Users</h2>

      {#if loading}
        <div class="mt-3 text-sm text-zinc-600">Loading…</div>
      {:else}
        <div class="mt-3 space-y-3">
          {#each users as u (u.id)}
            <div class="rounded-2xl border border-zinc-200 bg-zinc-50 p-4">
              {#if editingId === u.id}
                <div class="grid gap-2">
                  <label class="block">
                    <div class="text-xs font-semibold text-zinc-500">Username</div>
                    <input
                      class="mt-2 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 shadow-sm"
                      value={editUsername}
                      oninput={(e) => (editUsername = (e.currentTarget as HTMLInputElement).value)}
                    />
                  </label>

                  <label class="block">
                    <div class="text-xs font-semibold text-zinc-500">New temporary password</div>
                    <input
                      class="mt-2 w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 shadow-sm"
                      type="password"
                      placeholder="Leave blank to keep"
                      value={editPassword}
                      oninput={(e) => (editPassword = (e.currentTarget as HTMLInputElement).value)}
                    />
                    <div class="mt-2 text-xs text-zinc-500">
                      If set, the user will be required to choose a new password on their next
                      login.
                    </div>
                  </label>

                  <label class="flex items-center gap-2 text-sm text-zinc-800">
                    <input
                      type="checkbox"
                      checked={editIsAdmin}
                      onchange={(e) =>
                        (editIsAdmin = (e.currentTarget as HTMLInputElement).checked)}
                    />
                    Admin
                  </label>

                  <div class="mt-2 grid grid-cols-2 gap-2">
                    <button
                      type="button"
                      class="rounded-2xl border border-indigo-200 bg-indigo-50 px-3 py-2 text-sm font-semibold text-indigo-800 hover:bg-indigo-100 disabled:opacity-50"
                      disabled={saving || !editUsername.trim()}
                      onclick={() => void saveEdit()}
                    >
                      {saving ? 'Saving…' : 'Save'}
                    </button>
                    <button
                      type="button"
                      class="rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm font-semibold text-zinc-700 hover:bg-zinc-100"
                      onclick={() => cancelEdit()}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              {:else}
                <div class="flex items-center justify-between">
                  <div class="text-sm font-semibold">{u.username}</div>
                  <div class="flex items-center gap-2">
                    {#if u.must_change_password}
                      <Badge tone="amber" label="Password reset required" />
                    {/if}
                    <Badge
                      tone={u.is_admin ? 'indigo' : 'neutral'}
                      label={u.is_admin ? 'Admin' : 'User'}
                    />
                  </div>
                </div>

                <div class="mt-3 grid grid-cols-2 gap-2">
                  <button
                    type="button"
                    class="rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm font-semibold text-zinc-700 hover:bg-zinc-100"
                    onclick={() => startEdit(u)}
                  >
                    Edit
                  </button>
                  <button
                    type="button"
                    class="rounded-2xl border border-rose-200 bg-rose-50 px-3 py-2 text-sm font-semibold text-rose-700 hover:bg-rose-100"
                    onclick={() => void deleteUser(u)}
                    disabled={users.filter((x) => !x.deleted_at).length <= 1}
                  >
                    Delete
                  </button>
                </div>
              {/if}
            </div>
          {/each}
        </div>

        <div class="mt-4 text-xs text-zinc-500">BCTN supports up to 2 accounts total.</div>
      {/if}
    </section>

    {#if users.length < 2}
      <section class="rounded-3xl border border-zinc-200 bg-white p-5">
        <h2 class="text-sm font-semibold text-zinc-900">Create user</h2>
        <div class="mt-3 grid gap-2">
          <input
            class="w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 shadow-sm"
            placeholder="Username"
            value={createUsername}
            oninput={(e) => (createUsername = (e.currentTarget as HTMLInputElement).value)}
          />
          <input
            class="w-full rounded-2xl border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 shadow-sm"
            placeholder="Temporary password (min 8 chars)"
            type="password"
            value={createPassword}
            oninput={(e) => (createPassword = (e.currentTarget as HTMLInputElement).value)}
          />
          <label class="flex items-center gap-2 text-sm text-zinc-800">
            <input
              type="checkbox"
              checked={createIsAdmin}
              onchange={(e) => (createIsAdmin = (e.currentTarget as HTMLInputElement).checked)}
            />
            Admin
          </label>
          <div class="text-xs text-zinc-500">
            The new user will be required to change their password immediately after logging in.
          </div>
          <button
            type="button"
            class="mt-2 rounded-2xl border border-indigo-200 bg-indigo-50 px-3 py-2 text-sm font-semibold text-indigo-800 hover:bg-indigo-100 disabled:opacity-50"
            disabled={creating || !createUsername.trim() || createPassword.length < 8}
            onclick={() => void createUser()}
          >
            {creating ? 'Creating…' : 'Create'}
          </button>
        </div>
      </section>
    {/if}
  </div>
{/if}
