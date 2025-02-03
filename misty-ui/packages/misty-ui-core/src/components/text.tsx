import type { Style } from "../style";

export interface TextProps {
    text: string | number,
    style?: Style,
    onClick?: () => void
}

export function Text(props: TextProps) {
    const Tag = "m_text" as any

    return (
        <Tag {...props} />
    )
}