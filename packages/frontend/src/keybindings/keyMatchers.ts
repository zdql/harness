// ---------------------------------------------------------------------------
// keybindings/keyMatchers.ts
//
// Turns the KeyBindingConfig into a record of matcher functions, one per
// Command. Consumers call `matchers[Command.SUBMIT](key)` instead of doing
// raw key-property comparisons.
//
// Copied from gemini-cli (ui/key/keyMatchers.ts); `loadKeyMatchers()` was
// removed along with the keybindings file loader.
// ---------------------------------------------------------------------------

import type { Key } from "../keypress/useKeypress.ts";
import type { KeyBindingConfig } from "./keyBindings.ts";
import { Command, defaultKeyBindingConfig } from "./keyBindings.ts";

function matchCommand(
  command: Command,
  key: Key,
  config: KeyBindingConfig = defaultKeyBindingConfig,
): boolean {
  const bindings = config.get(command);
  if (!bindings) return false;
  return bindings.some((binding) => binding.matches(key));
}

type KeyMatcher = (key: Key) => boolean;

export type KeyMatchers = {
  readonly [C in Command]: KeyMatcher;
};

export function createKeyMatchers(
  config: KeyBindingConfig = defaultKeyBindingConfig,
): KeyMatchers {
  const matchers = {} as { [C in Command]: KeyMatcher };
  for (const command of Object.values(Command)) {
    matchers[command] = (key: Key) => matchCommand(command, key, config);
  }
  return matchers as KeyMatchers;
}

export const defaultKeyMatchers: KeyMatchers = createKeyMatchers(
  defaultKeyBindingConfig,
);

export { Command };
