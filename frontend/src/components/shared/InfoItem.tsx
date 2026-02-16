interface InfoItemProps {
  label: string
  value?: string
  breakAll?: boolean
  children?: React.ReactNode
}

export function InfoItem({ label, value, breakAll, children }: InfoItemProps) {
  return (
    <div>
      <p className="text-xs text-muted-foreground">{label}</p>
      {children ?? (
        <p className={`text-sm font-medium${breakAll ? " break-all" : ""}`}>
          {value}
        </p>
      )}
    </div>
  )
}
