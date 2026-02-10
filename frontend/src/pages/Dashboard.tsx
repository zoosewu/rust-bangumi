import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { useEffectQuery } from "@/hooks/useEffectQuery"
import { Effect } from "effect"
import { CoreApi } from "@/services/CoreApi"

export default function Dashboard() {
  const health = useEffectQuery(
    () =>
      Effect.gen(function* () {
        const api = yield* CoreApi
        return yield* api.getHealth
      }),
    [],
  )

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">Dashboard</h1>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Core Service
            </CardTitle>
          </CardHeader>
          <CardContent>
            {health.isLoading ? (
              <span className="text-muted-foreground">Checking...</span>
            ) : health.error ? (
              <Badge variant="destructive">Offline</Badge>
            ) : (
              <Badge className="bg-green-100 text-green-800">Online</Badge>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
