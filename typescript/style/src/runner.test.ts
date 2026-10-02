import { describe, expect, it } from 'vitest';

import type { ChildResult } from './runner.ts';
import { runSeam } from './runner.ts';

interface Call {
  commandArguments: string[];
  styleEnabled: boolean;
}

const recordingRunner = (...results: ChildResult[]) => {
  const calls: Call[] = [];

  const run = (commandArguments: string[], styleEnabled: boolean) => {
    calls.push({ commandArguments, styleEnabled });

    return results.shift() ?? { status: 0 };
  };

  return { calls, run };
};

describe('runSeam', () => {
  it('lints with house style, denies warnings, and forwards paths', () => {
    const { calls, run } = recordingRunner();

    expect(runSeam(['src', 'tests/a.ts'], run)).toBe(0);

    expect(calls).toEqual([
      { commandArguments: ['lint', '--deny-warnings', 'src', 'tests/a.ts'], styleEnabled: true },
    ]);
  });

  it('fixes, then formats the same paths without house style', () => {
    const { calls, run } = recordingRunner();

    expect(runSeam(['--fix', 'src'], run)).toBe(0);

    expect(calls).toEqual([
      { commandArguments: ['lint', '--deny-warnings', '--fix', 'src'], styleEnabled: true },
      { commandArguments: ['fmt', 'src'], styleEnabled: false },
    ]);
  });

  it('treats --fix after a path as a path', () => {
    const { calls, run } = recordingRunner();

    runSeam(['src', '--fix'], run);

    expect(calls).toEqual([
      { commandArguments: ['lint', '--deny-warnings', 'src', '--fix'], styleEnabled: true },
    ]);
  });

  it('formats even when lint fails, and the lint status wins', () => {
    const { calls, run } = recordingRunner({ status: 3 }, { status: 4 });

    expect(runSeam(['--fix'], run)).toBe(3);
    expect(calls).toHaveLength(2);
  });

  it('returns the format status when lint passes', () => {
    const { run } = recordingRunner({ status: 0 }, { status: 4 });

    expect(runSeam(['--fix'], run)).toBe(4);
  });

  it('reports 1 when a child ends without a status', () => {
    const { run } = recordingRunner({ status: null });

    expect(runSeam([], run)).toBe(1);
  });

  it('throws the spawn error and starts no formatter', () => {
    const spawnError = new Error('spawn failed');
    const { calls, run } = recordingRunner({ status: null, error: spawnError });

    expect(() => runSeam(['--fix'], run)).toThrow(spawnError);
    expect(calls).toHaveLength(1);
  });
});
