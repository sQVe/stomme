#!/usr/bin/env node
import { runSeam, runVitePlus } from './runner.ts';

process.exitCode = runSeam(process.argv.slice(2), runVitePlus);
