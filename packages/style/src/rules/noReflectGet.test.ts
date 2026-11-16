import { ruleTester } from '../../tests/ruleTester.ts';
import { noReflectGetRule } from './noReflectGet.ts';

ruleTester.run('no-reflect-get', noReflectGetRule, {
  valid: [
    "export const value = Reflect.has({ name: 'a' }, 'name');",
    'const Reflect = { get: () => 1 };\nexport const value = Reflect.get();',
    "export const value = ({ name: 'a' }).name;",
    `interface Person {
  readonly name: string;
  readonly greeting: string;
}

export const rename = (original: Person): Person =>
  new Proxy(original, {
    get(target, property, receiver) {
      if (property === 'name') {
        return 'proxied';
      }

      const value: unknown = Reflect.get(target, property, receiver);

      return value;
    },
  });`,
  ],
  invalid: [
    {
      code: "export const value = Reflect.get({ name: 'a' }, 'name');",
      errors: [{ messageId: 'reflectGet' }],
    },
    {
      code: "export const value = Reflect['get']({ name: 'a' }, 'name');",
      errors: [{ messageId: 'reflectGet' }],
    },
  ],
});
