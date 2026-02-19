<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import type { IconName } from '$lib/ui/icons';
  import { page } from '$app/stores';
  import { resolveRoute } from '$app/paths';

  const tabs = [
    { href: '/', label: 'Deck', icon: 'deck' },
    { href: '/explore', label: 'Explore', icon: 'explore' },
    { href: '/matches', label: 'Matches', icon: 'matches' },
    { href: '/likes', label: 'Likes', icon: 'likes' },
    { href: '/settings', label: 'Settings', icon: 'settings' },
  ] as const satisfies readonly {
    href: '/' | '/explore' | '/matches' | '/likes' | '/settings';
    label: string;
    icon: IconName;
  }[];
</script>

<nav
  class="fixed inset-x-0 bottom-0 z-40 border-t border-zinc-200 bg-white/95 backdrop-blur"
  style="padding-bottom: env(safe-area-inset-bottom);"
>
  <div class="mx-auto flex max-w-md items-stretch justify-between px-2">
    {#each tabs as tab (tab.href)}
      {@const active = $page.url.pathname === tab.href}
      <a
        href={resolveRoute(tab.href)}
        class="flex w-full flex-col items-center gap-1 px-2 py-2 text-xs"
        aria-current={active ? 'page' : undefined}
      >
        <Icon name={tab.icon} size={22} className={active ? 'text-indigo-600' : 'text-zinc-500'} />
        <span class={active ? 'text-indigo-600' : 'text-zinc-600'}>
          {tab.label}
        </span>
      </a>
    {/each}
  </div>
</nav>
