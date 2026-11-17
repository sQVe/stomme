import { expect, test } from 'vitest';

import { commandName, describeExtension, toolName } from './piExtension.ts';

test('the extension keeps its tool and command names', () => {
  expect(toolName).toBe('{{name}}');
  expect(commandName).toBe('{{name}}');
});

test('describeExtension names the tool and the command', () => {
  expect(describeExtension()).toBe('{{name}} registers the {{name}} command.');
});
