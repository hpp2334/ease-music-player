import { type ElementHandle } from './global'

export type { ElementHandle } from "./global"

export class Element {
    constructor(private handle: ElementHandle) {}
}

export { type Style } from './style'
export { type ViewProps, View } from './components/view'
export { type TextProps, Text } from './components/text'
