# Third-party notices

## anti-slop

These `seam` rules are ported from [dmmulroy/anti-slop](https://github.com/dmmulroy/anti-slop):

- `no-object-parameters`
- `no-reduce-accumulator-copy`
- `no-reflect-apply`
- `no-reflect-get`
- `no-unknown-type-aliases`
- `no-widen-then-assert`
- `require-safety-comment-for-type-assertion`

The source is the copy vendored in
[devdotfast/whiteboard](https://github.com/devdotfast/whiteboard), path `tools/oxlint/anti-slop`, at
commit `e2300beb51f5761fba95ca1cb8cd81a05b21f805`. The ports follow Seam's code style and shorten
some messages. `no-widen-then-assert` does not follow destructured `const` bindings.
`require-safety-comment-for-type-assertion` accepts only the `SAFETY:` marker and has no options.

- `no-reflect-get` allows `Reflect.get` with a receiver argument, which Proxy traps need.
- `no-reduce-accumulator-copy` skips method copies when the reducer's declared type admits a
  non-array.

Seam leaves out these upstream rules:

| Upstream rule                        | Reason                                                                                 |
| ------------------------------------ | -------------------------------------------------------------------------------------- |
| `no-unknown-parameters`              | Parsers take `unknown`, and `use-unknown-in-catch-callback-variable` requires it.      |
| `no-unknown-returns`                 | Readers return untyped JSON as `unknown` for a later validator.                        |
| `no-unsafe-dictionary-type`          | `Record<string, unknown>` is the common type for narrowing untyped JSON.               |
| `no-known-value-widening`            | Most fixes would add named types or `satisfies` without a clearer contract.            |
| `no-chained-type-assertions`         | `typescript/no-unsafe-type-assertion` already rejects the `unknown as T` step.         |
| `no-array-filter-map`                | Its fix needs iterator helpers, which a consumer's TypeScript lib may lack.            |
| `require-readable-spacing`           | `@stylistic/padding-line-between-statements` already sets blank lines.                 |
| `no-conditional-empty-object-spread` | A conditional spread is the usual way to add an optional property.                     |
| `no-module-mocking`                  | Some module mocks have no alternative, so consumers would need to turn the rule off.   |
| `no-shape-in-symbol-names`           | Not reviewed for Seam yet.                                                             |
| `no-runtime-typeof`                  | Not reviewed for Seam yet.                                                             |
| Effect rules                         | They apply only to projects that use Effect, and Seam's rules apply to every consumer. |

```text
MIT License

Copyright (c) 2026 Dillon Mulroy

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
