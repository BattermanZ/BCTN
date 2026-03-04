<script lang="ts">
  import BottomSheet from '../../BottomSheet.svelte';

  type Props = {
    open: boolean;
    onClose: () => void;
  };

  let { open, onClose }: Props = $props();
</script>

<BottomSheet {open} title="Details" {onClose}>
  <button type="button">First action</button>
  <button type="button">Second action</button>
</BottomSheet>
