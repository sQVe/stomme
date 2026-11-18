import { expect, test } from 'vitest';

import { greeting } from './greeting.ts';

test('greeting names the mod', () => {
  expect(greeting()).toBe('Hello from {{name}}.');
});
