import { bunRuntime } from './consumer.ts';
import { describePackage, loadingTests } from './packageSuite.ts';

describePackage(bunRuntime, [loadingTests]);
