import { fileURLToPath } from 'node:url';

import { createPackSetup } from '../../style/tests/packTarball.ts';

const importsPackageDirectory = fileURLToPath(new URL('../', import.meta.url));

export default createPackSetup('stomme-imports-pack-', importsPackageDirectory);
