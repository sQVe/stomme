import { expect, it } from 'vitest';

import { pinMismatches } from './pins.ts';

it('pins the tools Vite Plus depends on to the versions it declares', async () => {
  expect(await pinMismatches(new URL('../', import.meta.url))).toEqual([]);
});
