import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"

export interface Column<T> {
  key: string
  header: string
  render: (item: T) => React.ReactNode
}

interface DataTableProps<T> {
  columns: Column<T>[]
  data: T[]
  keyField: string
  onRowClick?: (item: T) => void
}

export function DataTable<T extends Record<string, unknown>>({
  columns,
  data,
  keyField,
  onRowClick,
}: DataTableProps<T>) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          {columns.map((col) => (
            <TableHead key={col.key}>{col.header}</TableHead>
          ))}
        </TableRow>
      </TableHeader>
      <TableBody>
        {data.map((item) => (
          <TableRow
            key={String(item[keyField])}
            onClick={() => onRowClick?.(item)}
            className={onRowClick ? "cursor-pointer" : ""}
          >
            {columns.map((col) => (
              <TableCell key={col.key}>{col.render(item)}</TableCell>
            ))}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
