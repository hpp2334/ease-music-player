import { Element, type ElementHandle } from "misty-ui-core"

interface Deno {
    core: {
        ops: {
            get_root_element_handle: () => ElementHandle
        }
    }
}

const Global = (globalThis as any).Deno as Deno

export function getRootElement(): Element {
    return new Element(Global.core.ops.get_root_element_handle())
}
