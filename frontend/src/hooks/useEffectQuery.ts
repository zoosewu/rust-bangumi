import { useState, useEffect, useCallback, useRef } from "react"
import type { Effect } from "effect"
import { AppRuntime } from "@/runtime/AppRuntime"

export function useEffectQuery<A>(
  effectFn: () => Effect.Effect<A, unknown, never>,
  deps: unknown[] = [],
) {
  const [data, setData] = useState<A | null>(null)
  const [error, setError] = useState<unknown>(null)
  const [isLoading, setIsLoading] = useState(true)
  const mountedRef = useRef(true)

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const execute = useCallback(() => {
    setIsLoading(true)
    setError(null)
    AppRuntime.runPromise(effectFn()).then(
      (result) => {
        if (mountedRef.current) {
          setData(result)
          setIsLoading(false)
        }
      },
      (err) => {
        if (mountedRef.current) {
          setError(err)
          setIsLoading(false)
        }
      },
    )
  }, deps)

  useEffect(() => {
    mountedRef.current = true
    execute()
    return () => {
      mountedRef.current = false
    }
  }, [execute])

  return { data, error, isLoading, refetch: execute }
}
