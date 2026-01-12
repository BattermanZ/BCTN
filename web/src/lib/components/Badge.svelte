<script lang="ts">
  type Tone = 'neutral' | 'teal' | 'rose' | 'amber' | 'indigo';

  type Props = { tone?: Tone; label: string };
  let { tone = 'neutral', label }: Props = $props();

  const toneClass: Record<Tone, string> = {
    neutral:
      'border-zinc-200 bg-white text-zinc-700 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-200',
    teal: 'border-teal-200 bg-teal-50 text-teal-700 dark:border-teal-900/60 dark:bg-teal-950/40 dark:text-teal-200',
    rose: 'border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-900/60 dark:bg-rose-950/40 dark:text-rose-200',
    amber:
      'border-amber-200 bg-amber-50 text-amber-800 dark:border-amber-900/60 dark:bg-amber-950/40 dark:text-amber-200',
    indigo:
      'border-indigo-200 bg-indigo-50 text-indigo-700 dark:border-indigo-900/60 dark:bg-indigo-950/40 dark:text-indigo-200',
  };
</script>

<span
  class={`inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] ${toneClass[tone]}`}
>
  {label}
</span>
