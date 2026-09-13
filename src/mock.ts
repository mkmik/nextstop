// ponytail: browser-only stand-in for the Rust backend so demo pages and headless screenshots work. Not shipped logic.
export async function mock<T>(cmd: string, _args: Record<string, unknown>): Promise<T> {
  switch (cmd) {
    case "log": return undefined as T;
    case "platform": return `${navigator.platform} (browser)` as T;
    default: throw new Error(`mock: no command ${cmd}`);
  }
}
