import { ruleTester } from '../../tests/ruleTester.ts';
import { requireSafetyCommentForTypeAssertionRule } from './requireSafetyCommentForTypeAssertion.ts';

const missing = { messageId: 'missingSafetyComment' };

ruleTester.run(
  'require-safety-comment-for-type-assertion',
  requireSafetyCommentForTypeAssertionRule,
  {
    valid: [
      "// SAFETY: The literal is a number.\nexport const value = JSON.parse('1') as number;",
      "export const names = ['a'] as const;",
      'export const read = (input: unknown) => /* SAFETY: Callers pass text. */ input as string;',
      `export const read = (input: unknown) => {
  // SAFETY: Callers pass text.
  return input as string;
};`,
      `export const count = (input: unknown) => {
  // SAFETY: Callers pass a number.
  for (let index = input as number; index > 0; index -= 1) {}
};`,
      `// oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The brand exists only in the type.
export const paneId = (number: number): PaneId => \`pane-\${number}\` as PaneId;`,
      `// eslint-disable-next-line typescript/no-unsafe-type-assertion -- The literal is a number.
export const value = JSON.parse('1') as number;`,
      `export const value = {
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The literal is a number.
  count: JSON.parse('1') as number,
};`,
      "export const value = JSON.parse('1') as number; // oxlint-disable-line typescript/no-unsafe-type-assertion -- The literal is a number.",
      `// oxlint-disable-next-line no-console, typescript/no-unsafe-type-assertion -- The literal is a number.
export const value = JSON.parse('1') as number;`,
      `/* oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The literal is a number. */
export const value = JSON.parse('1') as number;`,
      "export const value = JSON.parse('1') as number; /* oxlint-disable-line typescript/no-unsafe-type-assertion -- The literal is a number. */",
      `// oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The literal is a number.
export const value = (JSON.parse('1') as number) + JSON.parse(
  '2',
);`,
      `export const value = {
  count: (
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The literal is a number.
    JSON.parse('1') as number
  ),
};`,
    ],
    invalid: [
      {
        code: `// oxlint-disable-next-line typescript/no-unsafe-type-assertion -- This covers the statement's first line.
export const value = {
  count: JSON.parse('1') as number,
};`,
        errors: [missing],
      },
      {
        code: `export const value = {
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- This covers the property line.
  count: (
    JSON.parse('1') as number
  ),
};`,
        errors: [missing],
      },
      {
        code: "/* oxlint-disable typescript/no-unsafe-type-assertion -- The literal is a number. */\nexport const value = JSON.parse('1') as number;",
        errors: [missing],
      },
      {
        code: "// oxlint-disable-next-line typescript/no-unsafe-type-assertion\nexport const value = JSON.parse('1') as number;",
        errors: [missing],
      },
      {
        code: "// oxlint-disable-next-line typescript/no-unsafe-type-assertion --\nexport const value = JSON.parse('1') as number;",
        errors: [missing],
      },
      {
        code: "// oxlint-disable-next-line no-console -- The literal is a number.\nexport const value = JSON.parse('1') as number;",
        errors: [missing],
      },
      {
        code: "// oxlint-disable-next-line typescript/no-unsafe-type-assertion-extra -- The literal is a number.\nexport const value = JSON.parse('1') as number;",
        errors: [missing],
      },
      {
        code: "// oxlint-disable-next-line -- The literal is a number.\nexport const value = JSON.parse('1') as number;",
        errors: [missing],
      },
      {
        code: `// oxlint-disable-next-line typescript/no-unsafe-type-assertion -- The literal is a number.

export const value = JSON.parse('1') as number;`,
        errors: [missing],
      },
      {
        code: `// oxlint-disable-next-line typescript/no-unsafe-type-assertion -- This covers the first line.
export const first = JSON.parse('1') as number;
export const second = JSON.parse('2') as number;`,
        errors: [{ ...missing, line: 3 }],
      },
      {
        code: `export const value = {
  count: 1, // oxlint-disable-line typescript/no-unsafe-type-assertion -- This covers another line.
  total: JSON.parse('1') as number,
};`,
        errors: [missing],
      },
      { code: "export const value = JSON.parse('1') as number;", errors: [missing] },
      { code: "// SAFETY:\nexport const value = JSON.parse('1') as number;", errors: [missing] },
      {
        code: "// UNSAFETY: Not a marker.\nexport const value = JSON.parse('1') as number;",
        errors: [missing],
      },
      {
        code: `// SAFETY: The literal is a number.
export const first = JSON.parse('1') as number;
export const second = JSON.parse('2') as number;`,
        errors: [{ ...missing, line: 3 }],
      },
      { code: "export const value = <number>JSON.parse('1');", errors: [missing] },
      {
        code: `// SAFETY: This comment belongs to the function.
export const count = (input: unknown) => {
  for (let index = input as number; index > 0; index -= 1) {}
};`,
        errors: [missing],
      },
      {
        code: `// SAFETY: This comment belongs to the outer loop.
for (let outer = 0; outer < 2; outer += 1) {
  for (let inner = JSON.parse('1') as number; inner > 0; inner -= 1) {}
}`,
        errors: [missing],
      },
    ],
  },
);
