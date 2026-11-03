// Fixed local software identities. A mark does not establish compatibility.
const marks = Object.freeze({
  claude: 'assets/vendors/claude-spark-clay.svg',
  agentgateway: 'assets/vendors/agentgateway-mark.svg',
  cerbos: 'assets/vendors/cerbos-mark.png',
  permit: 'assets/vendors/permit-mark.svg',
  invariant: 'assets/vendors/invariant-mark.png',
});
export const VENDOR_MARK_IDS = Object.freeze(Object.keys(marks));
const sizes = new Set([20, 24, 32]);
export function vendorMark(name, size = 24) {
  if (typeof name !== 'string' || !Object.hasOwn(marks, name) || !sizes.has(size)) return '';
  return `<img class="vendor-mark" src="${marks[name]}" width="${size}" height="${size}" alt="" aria-hidden="true" decoding="async">`;
}
