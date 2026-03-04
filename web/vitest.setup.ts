import { cleanup } from '@testing-library/svelte';
import { afterEach } from 'vitest';

if (!Element.prototype.animate) {
  Element.prototype.animate = () =>
    ({
      cancel: () => {},
      play: () => {},
      pause: () => {},
      reverse: () => {},
      finish: () => {},
      onfinish: null,
      finished: Promise.resolve(),
    }) as Animation;
}

afterEach(() => {
  cleanup();
});
