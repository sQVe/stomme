#!/usr/bin/env node
import { runStickler, runVitePlus } from './runner.ts';

process.exitCode = runStickler(process.argv.slice(2), runVitePlus);
