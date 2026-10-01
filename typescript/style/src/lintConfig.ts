import { extname } from 'node:path';
import { fileURLToPath } from 'node:url';

import type { OxlintConfig } from 'vite-plus/lint';

type Rules = NonNullable<OxlintConfig['rules']>;

// The `stickler` command sets this for its lint child only, so editors keep ordinary diagnostics.
// eslint-disable-next-line node/no-process-env -- The style switch is the runner's contract with this config.
const styleEnabled = process.env.STICKLER_STYLE === '1';

export const testFiles = ['**/*.test.{ts,tsx}', '**/fixtures/**', 'tests/*.ts'];

// Oxlint resolves plugin specifiers from the consumer's config, where a strict package manager
// hides this package's dependencies. Absolute paths load the copies installed with this package.
const stylisticPlugin = fileURLToPath(import.meta.resolve('@stylistic/eslint-plugin'));

// The plugin sits beside this module: `.ts` in this repository and `.js` in the published package.
const sticklerPlugin = fileURLToPath(
  new URL(`./plugin${extname(import.meta.url)}`, import.meta.url),
);

const blockStatements = ['if', 'for', 'while', 'do', 'switch', 'try'];

const multilineStatements = [
  'multiline-block-like',
  'multiline-const',
  'multiline-export',
  'multiline-expression',
  'multiline-let',
  'multiline-return',
  'multiline-type',
  { selector: 'ClassDeclaration, TSInterfaceDeclaration', lineMode: 'multiline' },
];

const statementPadding = [
  { blankLine: 'always', prev: '*', next: ['return', 'break', 'continue'] },
  { blankLine: 'always', prev: '*', next: blockStatements },
  { blankLine: 'always', prev: blockStatements, next: '*' },
];

// Spread last: later entries win, so a multiline declaration is padded inside a declaration group.
const multilinePadding = [
  { blankLine: 'always', prev: '*', next: multilineStatements },
  { blankLine: 'always', prev: multilineStatements, next: '*' },
];

// Production code separates a group of declarations from the steps that use it; tests keep their
// arrange steps compact.
const declarationPadding = [
  { blankLine: 'always', prev: ['const', 'let'], next: '*' },
  { blankLine: 'any', prev: ['const', 'let'], next: ['const', 'let'] },
];

const paddingRule = (...entries: object[]): ['error', ...object[]] => ['error', ...entries];

const houseRules: Rules = {
  'stickler/naming-convention': 'error',
  'stickler/helper-before-use': 'error',
  'stickler/max-condition-checks': 'error',
  'stickler/type-placement': 'error',
  'stickler/no-abbreviations': 'error',
  'stickler/no-reference-comments': 'error',
  'stickler/require-disable-reason': 'error',
  'eslint/no-cond-assign': ['error', 'always'],
  'eslint/one-var': ['error', 'never'],
  '@stylistic/padding-line-between-statements': paddingRule(
    ...statementPadding,
    ...declarationPadding,
    ...multilinePadding,
  ),
};

const houseTestOverride = {
  files: testFiles,
  rules: {
    '@stylistic/padding-line-between-statements': paddingRule(
      ...statementPadding,
      ...multilinePadding,
    ),
  },
};

const ordinaryRules: Rules = {
  'typescript/no-unnecessary-condition': 'error',
  'typescript/prefer-readonly': 'error',
  'typescript/no-unsafe-type-assertion': 'error',
  'typescript/no-unnecessary-type-assertion': 'error',
  'typescript/no-unnecessary-boolean-literal-compare': 'error',
  'typescript/no-redundant-type-constituents': 'error',
  'typescript/no-meaningless-void-operator': 'error',
  'typescript/restrict-template-expressions': 'error',
  'typescript/no-base-to-string': 'error',
  'typescript/require-array-sort-compare': 'error',
  'typescript/no-mixed-enums': 'error',
  'typescript/strict-boolean-expressions': [
    'error',
    {
      allowNullableString: false,
      allowNullableBoolean: false,
      allowNullableObject: true,
      allowNumber: true,
    },
  ],
  'unicorn/prefer-top-level-await': 'error',
  'eslint/no-warning-comments': 'error',
  'typescript/unbound-method': 'error',
  'typescript/no-unsafe-enum-comparison': 'error',
  'typescript/explicit-module-boundary-types': 'error',
  'typescript/no-extraneous-class': 'off',
  'oxc/no-async-endpoint-handlers': 'off',
  'oxc/no-this-in-exported-function': 'off',

  'eslint/curly': 'error',
  'eslint/func-style': ['error', 'expression'],
  'eslint/prefer-const': 'error',
  'eslint/no-nested-ternary': 'error',
  'eslint/eqeqeq': [
    'error',
    'always',
    {
      null: 'ignore',
    },
  ],
  'eslint/no-param-reassign': 'error',
  'eslint/default-param-last': 'error',
  'eslint/no-empty': 'error',
  'eslint/no-unreachable-loop': 'error',
  'eslint/array-callback-return': 'error',
  'eslint/no-useless-assignment': 'error',
  'eslint/no-var': 'error',
  'eslint/no-return-assign': 'error',
  'eslint/no-multi-assign': 'error',
  'eslint/no-sequences': 'error',
  'eslint/no-loop-func': 'error',
  'eslint/no-promise-executor-return': 'error',
  'eslint/no-fallthrough': 'error',
  'eslint/no-magic-numbers': [
    'error',
    {
      ignore: [-1, 0, 1, 2],
      ignoreArrayIndexes: true,
      ignoreDefaultValues: true,
      ignoreClassFieldInitialValues: true,
      ignoreEnums: true,
      ignoreNumericLiteralTypes: true,
      ignoreReadonlyClassProperties: true,
      ignoreTypeIndexes: true,
    },
  ],

  'typescript/consistent-type-imports': 'error',
  'typescript/consistent-type-definitions': ['error', 'interface'],
  'typescript/no-inferrable-types': 'error',
  'typescript/prefer-nullish-coalescing': 'error',
  'typescript/no-explicit-any': 'error',
  'typescript/no-non-null-assertion': 'error',
  'typescript/no-misused-promises': 'error',
  'typescript/no-floating-promises': 'error',
  'typescript/await-thenable': 'error',
  'typescript/no-unsafe-argument': 'error',
  'typescript/no-unsafe-assignment': 'error',
  'typescript/no-unsafe-call': 'error',
  'typescript/no-unsafe-member-access': 'error',
  'typescript/no-unsafe-return': 'error',
  'typescript/switch-exhaustiveness-check': 'error',
  'typescript/only-throw-error': 'error',
  'typescript/return-await': 'error',
  'typescript/restrict-plus-operands': 'error',
  'typescript/ban-ts-comment': 'error',
  'typescript/no-unsafe-function-type': 'error',
  'typescript/prefer-includes': 'error',
  'typescript/prefer-promise-reject-errors': 'error',
  'typescript/prefer-ts-expect-error': 'error',
  'typescript/no-namespace': 'error',
  'typescript/no-require-imports': 'error',
  'typescript/consistent-generic-constructors': 'error',
  'typescript/array-type': [
    'error',
    {
      default: 'array',
    },
  ],
  'typescript/adjacent-overload-signatures': 'error',
  'typescript/no-empty-object-type': 'error',
  'typescript/no-import-type-side-effects': 'error',
  'typescript/no-non-null-asserted-nullish-coalescing': 'error',
  'typescript/use-unknown-in-catch-callback-variable': 'error',

  'import/no-cycle': 'error',
  'import/no-commonjs': 'error',
  'import/consistent-type-specifier-style': 'error',
  'import/first': 'error',
  'import/no-duplicates': 'error',
  'import/no-mutable-exports': 'error',
  'import/export': 'error',

  'unicorn/no-negation-in-equality-check': 'error',
  'unicorn/no-immediate-mutation': 'error',
  'unicorn/no-instanceof-array': 'error',
  'unicorn/no-this-assignment': 'error',
  'unicorn/no-unreadable-iife': 'error',
  'unicorn/no-useless-promise-resolve-reject': 'error',
  'unicorn/new-for-builtins': 'error',
  'unicorn/prefer-node-protocol': 'error',
  'unicorn/prefer-module': 'error',
  'unicorn/catch-error-name': [
    'error',
    {
      name: 'error',
    },
  ],
  'unicorn/error-message': 'error',
  'unicorn/throw-new-error': 'error',
  'unicorn/prefer-structured-clone': 'error',
  'unicorn/prefer-type-error': 'error',
  'unicorn/no-new-buffer': 'error',
  'unicorn/no-length-as-slice-end': 'error',
  'unicorn/no-abusive-eslint-disable': 'error',
  'unicorn/no-document-cookie': 'error',
  'unicorn/no-useless-error-capture-stack-trace': 'error',
  'unicorn/prefer-number-properties': 'error',
  'unicorn/prefer-array-find': 'error',
  'unicorn/prefer-array-flat-map': 'error',
  'unicorn/no-anonymous-default-export': 'error',

  'promise/always-return': ['error', { ignoreLastCallback: true }],
  'promise/catch-or-return': 'error',
  'promise/no-return-wrap': 'error',
  'promise/param-names': 'error',
  'promise/no-new-statics': 'error',
  'promise/valid-params': 'error',
  'promise/no-multiple-resolved': 'error',
  'promise/no-return-in-finally': 'error',
  // The suspicious category turns these on with the plugin. Both flag callback APIs that Node and
  // test runners still use, such as `done` and event handlers.
  'promise/no-callback-in-promise': 'off',
  'promise/no-promise-in-callback': 'off',

  'oxc/no-const-enum': 'error',
  'oxc/no-accumulating-spread': 'error',
  'oxc/bad-bitwise-operator': 'error',

  'node/no-new-require': 'error',
  'node/no-path-concat': 'error',
  'node/no-exports-assign': 'error',

  'eslint/no-console': 'warn',
  'eslint/complexity': [
    'warn',
    {
      max: 12,
    },
  ],
  'eslint/max-depth': [
    'warn',
    {
      max: 3,
    },
  ],
  'eslint/max-nested-callbacks': [
    'warn',
    {
      max: 3,
    },
  ],
  'eslint/no-await-in-loop': 'warn',
  'eslint/no-empty-function': 'warn',
  'eslint/no-template-curly-in-string': 'warn',
  'eslint/accessor-pairs': 'warn',

  'typescript/no-deprecated': 'warn',
  'typescript/require-await': 'warn',
  'typescript/no-confusing-void-expression': 'warn',
  'typescript/related-getter-setter-pairs': 'warn',
  'typescript/consistent-type-assertions': [
    'warn',
    {
      assertionStyle: 'as',
    },
  ],
  'typescript/prefer-for-of': 'warn',
  'typescript/prefer-function-type': 'warn',
  'typescript/prefer-reduce-type-parameter': 'warn',
  'typescript/unified-signatures': 'warn',
  'typescript/no-dynamic-delete': 'warn',
  'typescript/no-invalid-void-type': 'warn',
  'typescript/consistent-type-exports': 'warn',
  'typescript/no-unnecessary-type-conversion': 'warn',
  'typescript/no-unnecessary-type-parameters': 'warn',
  'typescript/no-useless-default-assignment': 'warn',
  'typescript/prefer-find': 'warn',
  'typescript/prefer-optional-chain': 'warn',
  'typescript/prefer-string-starts-ends-with': 'warn',

  'import/no-named-as-default': 'warn',
  'import/no-named-as-default-member': 'warn',
  'import/no-unassigned-import': 'warn',
  'import/no-named-default': 'warn',

  'unicorn/consistent-function-scoping': 'warn',
  'unicorn/no-object-as-default-parameter': 'warn',
  'unicorn/no-typeof-undefined': 'warn',
  'unicorn/no-unnecessary-array-flat-depth': 'warn',
  'unicorn/no-unnecessary-array-splice-count': 'warn',
  'unicorn/no-unnecessary-slice-end': 'warn',
  'unicorn/no-useless-switch-case': 'warn',
  'unicorn/prefer-array-some': 'warn',
  'unicorn/prefer-event-target': 'warn',
  'unicorn/prefer-regexp-test': 'warn',
  'unicorn/require-number-to-fixed-digits-argument': 'warn',
  'unicorn/consistent-date-clone': 'warn',
  'unicorn/consistent-existence-index-check': 'warn',
  'unicorn/custom-error-definition': 'warn',
  'unicorn/filename-case': [
    'warn',
    {
      cases: {
        camelCase: true,
        pascalCase: true,
      },
    },
  ],
  'unicorn/no-array-method-this-argument': 'warn',
  'unicorn/no-await-expression-member': 'warn',
  'unicorn/no-unreadable-array-destructuring': 'warn',
  'unicorn/no-useless-collection-argument': 'warn',
  'unicorn/prefer-array-index-of': 'warn',
  'unicorn/prefer-default-parameters': 'warn',
  'unicorn/prefer-global-this': 'warn',
  'unicorn/prefer-logical-operator-over-ternary': 'warn',
  'unicorn/prefer-negative-index': 'warn',
  'unicorn/prefer-object-from-entries': 'warn',
  'unicorn/prefer-optional-catch-binding': 'warn',
  'unicorn/prefer-string-trim-start-end': 'warn',
  'unicorn/prefer-response-static-json': 'warn',
  'unicorn/require-array-join-separator': 'warn',
  'unicorn/prefer-blob-reading-methods': 'warn',
  'unicorn/prefer-set-has': 'warn',
  'unicorn/no-instanceof-builtins': 'warn',

  'oxc/no-barrel-file': 'error',
  'oxc/no-map-spread': 'warn',
  'oxc/branches-sharing-code': 'warn',

  'node/no-process-env': 'warn',
};

const testOverride = {
  files: testFiles,
  rules: {
    'typescript/no-explicit-any': 'off',
    'typescript/no-non-null-assertion': 'off',
    'typescript/no-unsafe-type-assertion': 'off',
    'eslint/no-empty-function': 'off',
    'eslint/max-nested-callbacks': 'off',
    // Test steps are ordered, and async mocks need not suspend.
    'eslint/no-await-in-loop': 'off',
    'typescript/require-await': 'off',
    // Direct assertions and nested fixtures keep test scenarios together.
    'unicorn/no-await-expression-member': 'off',
    'eslint/complexity': 'off',
    'eslint/max-depth': 'off',
    // Tests isolate the real process environment.
    'node/no-process-env': 'off',
    // Assertions read methods off fakes and spies without calling them.
    'typescript/unbound-method': 'off',
    // Expected values, sizes, and timeouts read best inline.
    'eslint/no-magic-numbers': 'off',
  },
} satisfies NonNullable<OxlintConfig['overrides']>[number];

const houseConfig = {
  jsPlugins: [
    { name: '@stylistic', specifier: stylisticPlugin },
    { name: 'stickler', specifier: sticklerPlugin },
  ],
  rules: houseRules,
  overrides: [houseTestOverride],
};

export const lint: OxlintConfig = {
  plugins: ['typescript', 'unicorn', 'oxc', 'import', 'node', 'promise'],
  categories: {
    correctness: 'error',
    suspicious: 'error',
    perf: 'warn',
  },
  options: {
    typeAware: true,
    // Directives for house rules look unused when only ordinary rules load.
    ...(styleEnabled ? { reportUnusedDisableDirectives: 'error' } : {}),
  },
  jsPlugins: styleEnabled ? houseConfig.jsPlugins : [],
  rules: { ...(styleEnabled ? houseConfig.rules : {}), ...ordinaryRules },
  overrides: [...(styleEnabled ? houseConfig.overrides : []), testOverride],
};
