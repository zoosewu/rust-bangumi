import { CopyButton } from "./CopyButton"

interface MonospaceBlockProps {
  label: string
  text: string
  copyable?: boolean
  preWrap?: boolean
}

export function MonospaceBlock({ label, text, copyable = true, preWrap }: MonospaceBlockProps) {
  return (
    <div>
      <p className="text-xs text-muted-foreground mb-1">{label}</p>
      <div className={`bg-muted/50 rounded p-2${copyable ? " flex items-start gap-1" : ""}`}>
        <p
          className={`text-sm font-mono break-all${copyable ? " flex-1" : ""}${preWrap ? " whitespace-pre-wrap" : ""}`}
        >
          {text}
        </p>
        {copyable && <CopyButton text={text} />}
      </div>
    </div>
  )
}
