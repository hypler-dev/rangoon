// Fixed local presentation assets. A mark does not establish compatibility.
const marks = Object.freeze({ claude: 'assets/vendors/claude-spark-clay.svg' });
const sizes = new Set([20, 24, 32]);
export function vendorMark(name, size = 24) {
  if (!Object.hasOwn(marks, name) || !sizes.has(size)) return '';
  return `<img class="vendor-mark" src="${marks[name]}" width="${size}" height="${size}" alt="" aria-hidden="true" decoding="async">`;
}
