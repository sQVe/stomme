import { ruleTester } from '../../tests/ruleTester.ts';
import { requireDisableReasonRule } from './requireDisableReason.ts';

const missing = { messageId: 'missing' };

ruleTester.run('require-disable-reason', requireDisableReasonRule, {
  valid: [
    '// eslint-disable-next-line no-console -- The CLI prints its result.\nconsole.log(1);',
    '// oxlint-disable-next-line no-console -- The CLI prints its result.\nconsole.log(1);',
    'console.log(1); // eslint-disable-line no-console -- The CLI prints its result.',
    '/* eslint-disable no-console -- Generated output. */\nconsole.log(1);\n/* eslint-enable no-console */',
    '/* oxlint-disable -- Vendored file. */\nconsole.log(1);',
    '// eslint-enable no-console\nexport const a = 1;',
    '// Use eslint-disable comments sparingly.\nexport const a = 1;',
  ],
  invalid: [
    { code: '// eslint-disable-next-line no-console\nconsole.log(1);', errors: [missing] },
    { code: '// oxlint-disable-next-line no-console --\nconsole.log(1);', errors: [missing] },
    { code: '// eslint-disable-next-line no-console --   \nconsole.log(1);', errors: [missing] },
    { code: 'console.log(1); // eslint-disable-line no-console', errors: [missing] },
    { code: '/* eslint-disable */\nconsole.log(1);', errors: [missing] },
    { code: '/* oxlint-disable no-console */\nconsole.log(1);', errors: [missing] },
    {
      code: '// eslint-disable-next-line no-console - reason without the separator\nconsole.log(1);',
      errors: [missing],
    },
  ],
});
