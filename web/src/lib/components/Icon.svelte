<script lang="ts">
  import { iconPath, type IconName } from '$lib/ui/icons';

  type Props = { name: IconName; size?: number; className?: string };
  let { name, size = 22, className = '' }: Props = $props();
</script>

<svg
  viewBox="0 0 24 24"
  width={size}
  height={size}
  class={className}
  aria-hidden="true"
  fill="currentColor"
>
  <path d={iconPath(name)} />
</svg>
