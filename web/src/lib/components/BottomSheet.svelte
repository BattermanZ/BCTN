<script lang="ts">
  import type { Snippet } from 'svelte';
  import { tick } from 'svelte';
  import { fade, fly } from 'svelte/transition';

  type Props = {
    open?: boolean;
    title?: string;
    onClose?: () => void;
    children?: Snippet;
  };

  let { open = false, title, onClose = () => {}, children }: Props = $props();

  let panel = $state<HTMLDivElement | null>(null);
  let doneButton = $state<HTMLButtonElement | null>(null);
  let previousActive: HTMLElement | null = null;

  function close() {
    onClose();
  }

  function getFocusableWithin(root: HTMLElement): HTMLElement[] {
    const nodes = root.querySelectorAll<HTMLElement>(
      'button,[href],input,select,textarea,[tabindex]:not([tabindex="-1"])'
    );
    return Array.from(nodes).filter((n) => !n.hasAttribute('disabled') && n.tabIndex >= 0);
  }

  $effect(() => {
    if (!open) return;

    previousActive = document.activeElement instanceof HTMLElement ? document.activeElement : null;

    const ac = new AbortController();
    const { signal } = ac;

    void (async () => {
      await tick();
      doneButton?.focus();
    })();

    document.addEventListener(
      'keydown',
      (e) => {
        if (!open) return;

        if (e.key === 'Escape') {
          e.preventDefault();
          close();
          return;
        }

        if (e.key !== 'Tab') return;
        if (!panel) return;

        const focusable = getFocusableWithin(panel);
        if (!focusable.length) {
          e.preventDefault();
          doneButton?.focus();
          return;
        }

        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        const active = document.activeElement;

        if (e.shiftKey) {
          if (active === first || active === panel) {
            e.preventDefault();
            last.focus();
          }
        } else {
          if (active === last) {
            e.preventDefault();
            first.focus();
          }
        }
      },
      { signal }
    );

    return () => {
      ac.abort();
      previousActive?.focus();
      previousActive = null;
    };
  });
</script>

{#if open}
  <div class="fixed inset-0 z-50" aria-modal="true" role="dialog">
    <button
      type="button"
      tabindex="-1"
      class="absolute inset-0 cursor-default bg-black/35 dark:bg-black/55"
      onclick={close}
      transition:fade={{ duration: 120 }}
      aria-label="Close"
    ></button>

    <div
      bind:this={panel}
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
          bind:this={doneButton}
          type="button"
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
