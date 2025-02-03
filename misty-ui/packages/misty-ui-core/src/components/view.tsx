import type { Style } from "../style";

export interface ViewProps {
    style?: Style,
    children?: React.ReactElement | React.ReactElement[],
    onClick?: () => void
}

export function View(props: ViewProps) {
    const Tag = "m_view" as any
    return (
        <Tag {...props} />
    )
}