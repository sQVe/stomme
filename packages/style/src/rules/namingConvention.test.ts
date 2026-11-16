import { jsxRuleTester, ruleTester } from '../../tests/ruleTester.ts';
import { namingConventionRule } from './namingConvention.ts';

const camelCase = (name: string) => ({ messageId: 'name', data: { format: 'camelCase', name } });
const pascalCase = (name: string) => ({ messageId: 'name', data: { format: 'PascalCase', name } });

ruleTester.run('naming-convention', namingConventionRule, {
  valid: [
    'export interface RequestOptions { request_id: string }',
    'export type Result<Value> = Value | null;',
    'export class RequestError extends Error {}',
    "export const { request_id: requestId } = { request_id: 'one' };",
    'export const { omitted: _omitted, ...rest } = { omitted: 1, kept: 2 };',
    'export const unusedParameter = (_event: unknown, value: string) => value;',
    'export const anonymousParameter = (_: unknown, value: string) => value;',
    "import { snake_case } from './external.js';\n\nexport const value = snake_case;",
    "import { snake_case as snakeCase } from './external.js';\n\nexport const value = snakeCase;",
    "import { snake_case as SnakeCase } from './external.js';\n\nexport const value = new SnakeCase();",
    "import { snake_case as snake_case } from './external.js';\n\nexport const value = snake_case;",
    "import { 'kebab-name' as kebabName } from './external.js';\n\nexport const value = kebabName;",
    "import { type snake_case as SnakeCase } from './external.js';\n\nexport type Value = SnakeCase;",
    "import defaultValue from './external.js';\n\nexport const value = defaultValue;",
    "import DefaultClass from './external.js';\n\nexport const value = new DefaultClass();",
    "import * as external from './external.js';\n\nexport const value = external;",
    "import * as External from './external.js';\n\nexport const value = External;",
    "import external = require('./external.js');\n\nexport const value = external;",
    "import External = require('./external.js');\n\nexport const value = External;",
    'namespace Outer { export const member = 1; }\nimport member = Outer.member;\n\nexport const value = member;',
  ],
  invalid: [
    { code: 'export const MAX_RETRIES = 3;', errors: [camelCase('MAX_RETRIES')] },
    { code: 'export interface requestOptions {}', errors: [pascalCase('requestOptions')] },
    {
      code: 'export type result<value> = value | null;',
      errors: [pascalCase('result'), pascalCase('value')],
    },
    { code: 'export class requestError extends Error {}', errors: [pascalCase('requestError')] },
    {
      code: "export const { request_id } = { request_id: 'one' };",
      errors: [camelCase('request_id')],
    },
    {
      code: 'export const readParameter = (_value: string) => _value;',
      errors: [camelCase('_value')],
    },
    {
      code: 'export const badUnusedName = (_bad_name: unknown, value: string) => value;',
      errors: [camelCase('_bad_name')],
    },
    {
      code: 'export const { omitted: _bad_name, ...rest } = { omitted: 1, kept: 2 };',
      errors: [camelCase('_bad_name')],
    },
    {
      code: "import { value as BAD_NAME } from './external.js';\n\nexport const used = BAD_NAME;",
      errors: [camelCase('BAD_NAME')],
    },
    {
      code: "import { value as bad_name } from './external.js';\n\nexport const used = bad_name;",
      errors: [camelCase('bad_name')],
    },
    {
      code: "import { 'kebab-name' as kebab_name } from './external.js';\n\nexport const used = kebab_name;",
      errors: [camelCase('kebab_name')],
    },
    {
      code: "import { type Value as BAD_TYPE } from './external.js';\n\nexport type Used = BAD_TYPE;",
      errors: [camelCase('BAD_TYPE')],
    },
    {
      code: "import DEFAULT_THING from './external.js';\n\nexport const used = DEFAULT_THING;",
      errors: [camelCase('DEFAULT_THING')],
    },
    {
      code: "import * as bad_namespace from './external.js';\n\nexport const used = bad_namespace;",
      errors: [camelCase('bad_namespace')],
    },
    {
      code: "import bad_name = require('./external.js');\n\nexport const used = bad_name;",
      errors: [camelCase('bad_name')],
    },
    {
      code: 'namespace Outer { export const member = 1; }\nimport BAD_MEMBER = Outer.member;\n\nexport const used = BAD_MEMBER;',
      errors: [camelCase('BAD_MEMBER')],
    },
  ],
});

jsxRuleTester.run('naming-convention with JSX components', namingConventionRule, {
  valid: [
    'export function DeclaredView() { return <box />; }',
    'export const ArrowView = () => <box>text</box>;',
    'export const ExpressionView = function () { return (<><box /></>); };',
    `export const LoadingView = (loading: boolean) => {
      if (loading) { return null; }
      return loading ? <text /> : <box />;
    };`,
    'export const OptionalView = (shown: boolean) => shown && <box />;',
    'export const renderView = () => <box />;',
    "import { memo } from 'react';\n\nexport const MemoView = memo(() => <box />);",
    "import { memo } from 'react';\n\nconst View = () => <box />;\nexport const MemoizedView = memo(View);",
    "import { forwardRef } from 'react';\n\nexport const InputView = forwardRef((props: object, ref) => <input {...props} ref={ref} />);",
    "import React from 'react';\n\nexport const ReactMemoView = React.memo(() => <box />);",
    "import React from 'react';\n\nexport const ReactInputView = React.forwardRef((props: object, ref) => <input {...props} ref={ref} />);",
    "import { forwardRef, memo } from 'react';\n\nexport const NestedView = memo(forwardRef((props: object, ref) => <input {...props} ref={ref} />));",
    "import { memo } from 'react';\n\nexport const memoizedValue = memo(() => <box />);",
    "import { memo as reactMemo } from 'react';\n\nexport const AliasView = reactMemo(() => <box />);",
    "import * as R from 'react';\n\nexport const NamespaceView = R.memo(() => <box />);",
    "import * as R from 'react';\n\nexport const NamespaceInput = R.forwardRef((props: object, ref) => <input {...props} ref={ref} />);",
    "import Preact from 'react';\n\nexport const DefaultView = Preact.memo(() => <box />);",
    'const EmptyView = () => null;\nexport const Page = () => <EmptyView />;',
    'function HiddenView() { return null; }\nexport const Screen = () => <><HiddenView /></>;',
    'export const Button = ({ icon: Icon }: { icon: () => null }) => <Icon />;',
    'export function Row(Cell: () => null) { return <Cell />; }',
    'const icons = { check: () => null };\nconst Icon = icons.check;\nexport const Badge = () => <Icon />;',
    "import { lazy } from 'react';\n\nconst View = lazy(() => import('./view.js'));\nexport const Page = () => <View />;",
    "import styled from 'styled-components';\n\nconst Box = styled.div;\nexport const Card = () => <Box />;",
    'let MutableEmpty = () => null;\nMutableEmpty = () => null;\nexport const Page = () => <MutableEmpty />;',
    'export function Row(Cell: () => null, plain: boolean) { if (plain) { Cell = () => null; } return <Cell />; }',
    "import { view as View } from './view.js';\n\nexport const Page = () => <View />;",
    "import View from './view.js';\n\nexport const Page = () => <View />;",
  ],
  invalid: [
    { code: 'export const MaxItems = 3;', errors: [camelCase('MaxItems')] },
    { code: 'export const MAX_ITEMS = 3;', errors: [camelCase('MAX_ITEMS')] },
    { code: 'export function NotView() { return 1; }', errors: [camelCase('NotView')] },
    {
      code: 'export const Renderer = () => { const render = () => <box />; return render; };',
      errors: [camelCase('Renderer')],
    },
    { code: 'export const MAIN_VIEW = () => <box />;', errors: [pascalCase('MAIN_VIEW')] },
    { code: 'export let MutableView = () => <box />;', errors: [camelCase('MutableView')] },
    {
      code: 'export var VariableView = function () { return <box />; };',
      errors: [camelCase('VariableView')],
    },
    {
      code: 'export const { name: DisplayName } = () => <box />;',
      errors: [camelCase('DisplayName')],
    },
    {
      code: 'export const { length: Arity } = function () { return <box />; };',
      errors: [camelCase('Arity')],
    },
    {
      code: "import { memo } from 'react';\n\nexport const MEMO_VIEW = memo(() => <box />);",
      errors: [pascalCase('MEMO_VIEW')],
    },
    {
      code: "import { memo } from 'react';\n\nexport let MutableMemo = memo(() => <box />);",
      errors: [camelCase('MutableMemo')],
    },
    {
      code: "import { memo } from 'react';\n\nexport const { type: MemoType } = memo(() => <box />);",
      errors: [camelCase('MemoType')],
    },
    {
      code: 'const memo = (value: number) => value;\nexport const MaxItems = memo(3);',
      errors: [camelCase('MaxItems')],
    },
    {
      code: "import { memo } from './cache.js';\n\nexport const MaxItems = memo(3);",
      errors: [camelCase('MaxItems')],
    },
    {
      code: "import React from './react.js';\n\nexport const MaxItems = React.memo(3);",
      errors: [camelCase('MaxItems')],
    },
    {
      code: "import { memo as forwardRef } from './cache.js';\n\nexport const MaxItems = forwardRef(3);",
      errors: [camelCase('MaxItems')],
    },
    {
      code: "import { useMemo as memo } from 'react';\n\nexport const MaxItems = memo(() => 3, []);",
      errors: [camelCase('MaxItems')],
    },
    {
      code: 'export const React = { memo: (value: number) => value };\nexport const MaxItems = React.memo(3);',
      errors: [camelCase('React'), camelCase('MaxItems')],
    },
    {
      code: 'const wrap = (render: () => unknown) => render;\nexport const Wrapped = wrap(() => <box />);',
      errors: [camelCase('Wrapped')],
    },
    {
      code: 'export const Helper = () => null;\nexport const page = () => <helper />;',
      errors: [camelCase('Helper')],
    },
    {
      code: 'export const pick = (Fallback: () => null) => { Fallback = () => null; return Fallback; };',
      errors: [camelCase('Fallback')],
    },
    {
      code: 'const icons = { Check: () => null };\nexport const Icons = icons;\nexport const Badge = () => <Icons.Check />;',
      errors: [camelCase('Icons')],
    },
    {
      code: 'export const shown = (Box: unknown) => Box;\nexport const Page = () => { const Box = () => null; return <Box />; };',
      errors: [camelCase('Box')],
    },
    {
      code: 'export const Lower = 1;\nexport const Page = () => <lower />;',
      errors: [camelCase('Lower')],
    },
    {
      code: 'export function ParameterView(Label: string) { return <box>{Label}</box>; }',
      errors: [camelCase('Label')],
    },
    {
      code: 'export function DestructuredView({ title: Title }: { title: string }) { return <box>{Title}</box>; }',
      errors: [camelCase('Title')],
    },
    {
      code: "import { View as Main_View } from './view.js';\n\nexport const Page = () => <Main_View />;",
      errors: [pascalCase('Main_View')],
    },
    {
      code: "import MAIN_VIEW from './view.js';\n\nexport const Page = () => <MAIN_VIEW />;",
      errors: [pascalCase('MAIN_VIEW')],
    },
  ],
});
