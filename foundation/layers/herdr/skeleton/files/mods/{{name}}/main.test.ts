import { expect, test } from 'vitest';

import { describeMod, modName } from './main.ts';

test('the mod keeps its name', () => {
  expect(modName).toBe('{{name}}');
});

test('describeMod names the mod', () => {
  expect(describeMod()).toBe('The {{name}} mod is loaded.');
});
