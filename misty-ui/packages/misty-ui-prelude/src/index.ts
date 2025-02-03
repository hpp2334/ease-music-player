interface Deno {
    core: {
        ops: {
            set_timeout: (f: () => void, ms: number) => number,
            console_log: (args: any[]) => void,
        }
    }
}

const _Deno = (globalThis as any).Deno as Deno

globalThis.console ??= {
    log(...msg) {
        _Deno.core.ops.console_log(msg)
    },
}
globalThis.setTimeout ??= function (f, ms) {
    return _Deno.core.ops.set_timeout(f, ms ?? 0)
}

