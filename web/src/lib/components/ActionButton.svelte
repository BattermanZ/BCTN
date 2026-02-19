<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import type { IconName } from '$lib/ui/icons';

  type Tone = 'neutral' | 'teal' | 'rose' | 'amber';

  type Props = {
    label: string;
    icon?: IconName | null;
    tone?: Tone;
    onClick?: () => void;
  };

  let { label, icon = null, tone = 'neutral', onClick = () => {} }: Props = $props();

  const toneClass: Record<Tone, string> = {
    neutral: 'border-zinc-200 bg-white text-zinc-900 hover:bg-zinc-50',
    teal: 'border-teal-200 bg-teal-50 text-teal-800 hover:bg-teal-100',
    rose: 'border-rose-200 bg-rose-50 text-rose-800 hover:bg-rose-100',
    amber: 'border-amber-200 bg-amber-50 text-amber-900 hover:bg-amber-100',
  };
</script>

<button
  class={`inline-flex items-center justify-center gap-2 rounded-full border px-4 py-3 text-sm font-semibold shadow-sm ${toneClass[tone]}`}
  onclick={onClick}
  type="button"
>
  {#if icon}
    <Icon name={icon} size={18} className="opacity-90" />
  {/if}
  <span>{label}</span>
</button>
