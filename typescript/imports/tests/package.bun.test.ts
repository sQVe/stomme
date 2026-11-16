import { bunRuntime } from './consumer.ts';
import { describePackage, loadingTests } from './packageSuite.ts';

// Bun runs only the test that loads the plugin; lint results match Node's.
describePackage(bunRuntime, [loadingTests]);
