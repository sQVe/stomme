import { bunRuntime } from './consumer.ts';
import { describePackage, loadingTests } from './packageSuite.ts';

// Bun runs only the tests that load the package; lint and fix results match Node's.
describePackage(bunRuntime, [loadingTests]);
