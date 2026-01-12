<script lang="ts">
  type Tone = 'neutral' | 'teal' | 'rose' | 'amber' | 'indigo';

  type Props = { tone?: Tone; label: string };
  let { tone = 'neutral', label }: Props = $props();

  const toneClass: Record<Tone, string> = {
    neutral: 'border-zinc-200 bg-white text-zinc-700',
    teal: 'border-teal-200 bg-teal-50 text-teal-700',
    rose: 'border-rose-200 bg-rose-50 text-rose-700',
    amber: 'border-amber-200 bg-amber-50 text-amber-800',
    indigo: 'border-indigo-200 bg-indigo-50 text-indigo-700',
  };
</script>

<span
  class={`inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] ${toneClass[tone]}`}
>
  {label}
</span>
