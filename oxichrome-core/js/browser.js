// The one place oxichrome tells Firefox from Chrome. Firefox is the only browser with
// `browser.runtime.getBrowserInfo`; every other snippet imports this instead of checking itself.
export const isFirefox = typeof globalThis.browser?.runtime?.getBrowserInfo === 'function';

export function is_firefox() {
    return isFirefox;
}
