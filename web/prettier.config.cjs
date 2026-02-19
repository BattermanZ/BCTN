/** @type {import("prettier").Config} */
module.exports = {
  plugins: ['prettier-plugin-svelte', 'prettier-plugin-tailwindcss'],
  printWidth: 100,
  singleQuote: true,
  trailingComma: 'es5',
  svelteSortOrder: 'options-scripts-markup-styles',
  svelteStrictMode: false,
};
