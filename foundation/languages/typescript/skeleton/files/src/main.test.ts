import { expect, test } from 'vitest';

import { greet } from './main.ts';

test('greet names the person it greets', () => {
  expect(greet('{{name}}')).toBe('Hello, {{name}}.');
});
