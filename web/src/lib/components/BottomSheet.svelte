<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fade, fly } from 'svelte/transition';

  type Props = {
    open?: boolean;
    title?: string;
    onClose?: () => void;
    children?: Snippet;
  };

  let { open = false, title, onClose = () => {}, children }: Props = $props();

  function close() {
    onClose();
  }
</script>

{#if open}
  <div class="fixed inset-0 z-50" aria-modal="true" role="dialog">
    <button
      class="absolute inset-0 cursor-default bg-black/35 dark:bg-black/55"
      onclick={close}
      transition:fade={{ duration: 120 }}
      aria-label="Close"
    ></button>

    <div
      class="absolute inset-x-0 bottom-0 mx-auto max-w-md rounded-t-3xl border border-zinc-200 bg-white shadow-2xl dark:border-zinc-800 dark:bg-zinc-950"
      style="padding-bottom: env(safe-area-inset-bottom);"
      transition:fly={{ y: 24, duration: 160 }}
    >
      <div class="flex items-center justify-between px-5 pb-2 pt-4">
        <div class="min-w-0">
          {#if title}
            <h2 class="truncate text-base font-semibold text-zinc-900 dark:text-zinc-100">
              {title}
            </h2>
          {/if}
        </div>
        <button
          class="rounded-full border border-zinc-200 bg-white px-3 py-1.5 text-sm font-medium text-zinc-700 hover:bg-zinc-50 dark:border-zinc-800 dark:bg-zinc-950 dark:text-zinc-200 dark:hover:bg-zinc-900"
          onclick={close}
        >
          Done
        </button>
      </div>

      <div class="max-h-[70vh] overflow-auto px-5 pb-6">
        {@render children?.()}
      </div>
    </div>
  </div>
{/if}
