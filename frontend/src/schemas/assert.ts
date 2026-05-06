type DeepReadonly<T> = T extends (infer U)[]
  ? readonly DeepReadonly<U>[]
  : T extends object
    ? { readonly [K in keyof T]: DeepReadonly<T[K]> }
    : T

// Effect Schema decoded structs and arrays are readonly, while OpenAPI generator
// output uses mutable object and array properties. Normalize generated types
// before checking that schema types remain API-compatible.
export type AssertExtends<_Generated, _Schema extends DeepReadonly<_Generated>> = true
