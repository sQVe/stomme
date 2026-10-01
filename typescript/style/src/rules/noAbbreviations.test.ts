import { jsxRuleTester, ruleTester } from '../fixtures/ruleTester.ts';
import { noAbbreviationsRule } from './noAbbreviations.ts';

const abbreviation = (name: string, word: string, replacement: string) => ({
  messageId: 'abbreviation',
  data: { name, word, replacement },
});

ruleTester.run('no-abbreviations', noAbbreviationsRule, {
  valid: [
    'export const button = 1;',
    'export const errorMessage = "failed";',
    'export const stringify = (value: unknown) => JSON.stringify(value);',
    'export const resolveRequest = (request: Request) => request;',
    'export const elementIndex = 0;',
    'export const userId = 1;',
    'export const baseUrl = "https://example.com";',
    'export const readEnv = (env: Record<string, string>) => env;',
    'export const parseArgs = (args: string[]) => args;',
    'export const render = (props: { ref: unknown }) => props.ref;',
    'export const sum = (values: number[]) => { let total = 0; for (let i = 0; i < values.length; i++) { total += values[i] ?? 0; } return total; };',
    "import { cb, errMsg } from './external.js';\n\nexport const callback = [cb, errMsg];",
    "import * as fn from './external.js';\n\nexport const external = fn;",
    'export const { cfg: config } = { cfg: 1 };',
    'export const options = { opts: 1, btn: 2 };',
    'export interface Settings { ctx: unknown; req(): void }',
    'export const read = (input: { res: number }) => input.res;',
    'export const parseHTMLElement = () => 1;',
  ],
  invalid: [
    { code: 'export const btn = 1;', errors: [abbreviation('btn', 'btn', 'button')] },
    {
      code: 'export const errMsg = "failed";',
      errors: [abbreviation('errMsg', 'err', 'error'), abbreviation('errMsg', 'msg', 'message')],
    },
    {
      code: 'export const onBtnClick = () => 1;',
      errors: [abbreviation('onBtnClick', 'btn', 'button')],
    },
    {
      code: 'export const handle = (req: Request, res: Response) => [req, res];',
      errors: [abbreviation('req', 'req', 'request'), abbreviation('res', 'res', 'response')],
    },
    {
      code: 'export const run = (_ctx: unknown) => 1;',
      errors: [abbreviation('_ctx', 'ctx', 'context')],
    },
    {
      code: 'export const RETRY_CNT = 3;',
      errors: [abbreviation('RETRY_CNT', 'cnt', 'count')],
    },
    { code: 'export class ReqHandler {}', errors: [abbreviation('ReqHandler', 'req', 'request')] },
    {
      code: 'export interface CfgShape { name: string }',
      errors: [abbreviation('CfgShape', 'cfg', 'config')],
    },
    {
      code: 'export type ValMap<Obj> = Map<string, Obj>;',
      errors: [abbreviation('ValMap', 'val', 'value'), abbreviation('Obj', 'obj', 'object')],
    },
    { code: 'export enum BtnKind { Primary }', errors: [abbreviation('BtnKind', 'btn', 'button')] },
    {
      code: 'export const { cfg } = { cfg: 1 };',
      errors: [abbreviation('cfg', 'cfg', 'config')],
    },
    {
      code: 'export const read = () => { try { return 1; } catch (err) { return err; } };',
      errors: [abbreviation('err', 'err', 'error')],
    },
    {
      code: 'export function tmpPath(str: string) { return str; }',
      errors: [abbreviation('tmpPath', 'tmp', 'temporary'), abbreviation('str', 'str', 'text')],
    },
  ],
});

jsxRuleTester.run('no-abbreviations with JSX', noAbbreviationsRule, {
  valid: ['export const Button = (props: { label: string }) => <box>{props.label}</box>;'],
  invalid: [
    {
      code: 'export const BtnView = () => <box />;',
      errors: [abbreviation('BtnView', 'btn', 'button')],
    },
  ],
});
