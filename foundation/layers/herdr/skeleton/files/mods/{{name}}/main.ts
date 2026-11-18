import { div, View } from 'gpui-kit';
import type { Element } from 'gpui-kit';

import { greeting } from './greeting.ts';

export default class Mod extends View {
  render(): Element {
    return div().child(greeting());
  }
}
