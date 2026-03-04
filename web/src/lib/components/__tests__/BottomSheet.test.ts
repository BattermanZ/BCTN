import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import BottomSheetHost from './fixtures/BottomSheetHost.svelte';

describe('BottomSheet', () => {
  it('calls onClose when Escape is pressed', async () => {
    const onClose = vi.fn();

    render(BottomSheetHost, { open: true, onClose });

    await fireEvent.keyDown(document, { key: 'Escape' });

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('traps focus inside the sheet panel', async () => {
    const onClose = vi.fn();

    render(BottomSheetHost, { open: true, onClose });

    const done = await screen.findByRole('button', { name: 'Done' });
    const secondAction = screen.getByRole('button', { name: 'Second action' });

    await waitFor(() => {
      expect(document.activeElement).toBe(done);
    });

    secondAction.focus();
    await fireEvent.keyDown(document, { key: 'Tab' });
    expect(document.activeElement).toBe(done);

    await fireEvent.keyDown(document, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(secondAction);
  });
});
