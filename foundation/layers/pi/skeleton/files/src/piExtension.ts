import type { ExtensionAPI } from '@earendil-works/pi-coding-agent';

export const toolName = '{{name}}';

export const commandName = '{{name}}';

export const describeExtension = (): string => `${toolName} registers the ${commandName} command.`;

const registerExtension = (pi: Pick<ExtensionAPI, 'registerCommand'>): void => {
  pi.registerCommand(commandName, {
    description: describeExtension(),
    handler: (_commandArguments, context) => {
      context.ui.notify(describeExtension(), 'info');

      return Promise.resolve();
    },
  });
};

export default registerExtension;
