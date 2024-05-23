/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export interface SignatureOptions {
  minSize: number
  avgSize: number
  maxSize: number
}
/** Writes calculated signature for `source` to the `dest`. */
export function signatureToFile(source: string, dest: string, options?: SignatureOptions | undefined | null): void
/** Returns calculated signature of the `source`. */
export function signature(source: string, options?: SignatureOptions | undefined | null): Buffer
/** Writes a diff that transforms `a` -> `b` into `dest`. */
export function diff(a: string, b: string, dest: string, options?: SignatureOptions | undefined | null): void
/** Applies `diff_path` to the `a` and writes the result to `result`. */
export function apply(diffPath: string, a: string, result: string): void
