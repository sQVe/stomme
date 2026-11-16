import { nodeRuntime } from './consumer.ts';
import { behaviorTests, describePackage, loadingTests } from './packageSuite.ts';

describePackage(nodeRuntime, [loadingTests, behaviorTests]);
