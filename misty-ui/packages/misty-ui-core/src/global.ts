
const symIndent = Symbol()

type Opaque<Identifier extends string> = unknown & { [symIndent]: Identifier }

export type ElementHandle = Opaque<'ElementHandle'>
