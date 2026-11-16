import { ruleTester } from '../../tests/ruleTester.ts';
import { noUnnamedArgumentsRule } from './noUnnamedArguments.ts';

ruleTester.run('no-unnamed-arguments', noUnnamedArgumentsRule, {
  valid: [
    `const status = isOpen ? 'open' : 'closed';

    send(status);`,
    `const names = items.filter(isOpen).map(toName);

    send(names);`,
    'send({ ...(model === undefined ? {} : { model }) });',
    'send({ ...(model === undefined ? { model } : {}) });',
    'send(getUser());',
    'send(user.name());',
    'send(user.profile.name());',
    'send(getUser().name);',
    'items.filter(isOpen).map(toName);',
    'items.map((item) => (item.open ? 1 : 2));',
    'items.map((item) => item.open ? 1 : 2);',
    'send({ key: 1 });',
    `const label = ok ? a : b; send(\`\${label}\`);`,
  ],
  invalid: [
    {
      code: "send(isOpen ? 'open' : 'closed');",
      errors: [
        {
          message: 'Assign this ternary to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send(items.filter(isOpen).map(toName));',
      errors: [
        {
          message: 'Assign this call chain to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'new Report(a.b().c());',
      errors: [
        {
          message: 'Assign this call chain to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send((flag ? a : b) as string);',
      errors: [
        {
          message: 'Assign this ternary to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send(a?.b()?.c());',
      errors: [
        {
          message: 'Assign this call chain to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send({ status: flag ? a : b });',
      errors: [
        {
          message: 'Assign this ternary to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send([flag ? a : b]);',
      errors: [
        {
          message: 'Assign this ternary to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send(...(flag ? a : b));',
      errors: [
        {
          message: 'Assign this ternary to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send((a?.b()).c());',
      errors: [
        {
          message: 'Assign this call chain to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send({ ...make().value() });',
      errors: [
        {
          message: 'Assign this call chain to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send({ ...(flag ? { left } : { right }) });',
      errors: [
        {
          message: 'Assign this ternary to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send(make().value.method());',
      errors: [
        {
          message: 'Assign this call chain to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send(flag && (ok ? a : b));',
      errors: [
        {
          message: 'Assign this ternary to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send(count + (ok ? 1 : 2));',
      errors: [
        {
          message: 'Assign this ternary to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send(!(ok ? a : b));',
      errors: [
        {
          message: 'Assign this ternary to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: `send(\`\${ok ? a : b}\`);`,
      errors: [
        {
          message: 'Assign this ternary to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send({ [ok ? a : b]: 1 });',
      errors: [
        {
          message: 'Assign this ternary to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: `send(\`\${items.filter(isOpen).map(toName)}\`);`,
      errors: [
        {
          message: 'Assign this call chain to a named variable before passing it as an argument.',
        },
      ],
    },
    {
      code: 'send(items.filter(isOpen).length > 0);',
      errors: [
        {
          message: 'Assign this call chain to a named variable before passing it as an argument.',
        },
      ],
    },
  ],
});
