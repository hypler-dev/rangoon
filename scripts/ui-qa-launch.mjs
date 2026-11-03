/** Select an installed browser launch without downloads or shell interpretation. */
export function uiQaLaunch({ platform, hostArm64, browserPath, explicitOverride, args }) {
  if (typeof browserPath !== 'string' || !browserPath || !Array.isArray(args) || args.some(arg => typeof arg !== 'string')) {
    throw new TypeError('Invalid browser launch inputs.');
  }
  if (platform === 'darwin' && typeof hostArm64 !== 'boolean') {
    throw new TypeError('macOS host architecture must be established before launch.');
  }
  if (platform === 'darwin' && hostArm64 && explicitOverride !== true) {
    return { command: '/usr/bin/arch', args: ['-arm64', browserPath, ...args], selection: 'default_macos_arm64' };
  }
  return { command: browserPath, args: [...args], selection: explicitOverride === true ? 'explicit_browser_override' : 'direct_host_default' };
}
