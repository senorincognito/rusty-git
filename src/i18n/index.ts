/**
 * The app's texts. Everything the user reads lives in `en.ts`; components use `t.<section>.<key>`
 * (a string, or a function for texts with numbers or names in them) and `fill` for sentences with markup.
 * Only English exists; this keeps the wording in one place.
 */
export { en as t } from "./en";
export { fill } from "./fill";
