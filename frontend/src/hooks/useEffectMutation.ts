import { useState, useCallback, useRef, useEffect } from "react"
import type { Effect } from "effect"
import { AppRuntime } from "@/runtime/AppRuntime"

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function useEffectMutation<Args extends unknown[], A>(
  effectFn: (...args: Args) => Effect.Effect<A, any, any>,
) {
  const [data, setData] = useState<A | null>(null)
  const [error, setError] = useState<unknown>(null)
  const [isLoading, setIsLoading] = useState(false)
  const mountedRef = useRef(true)

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  const mutate = useCallback(
    (...args: Args) => {
      setIsLoading(true)
      setError(null)
      return AppRuntime.runPromise(effectFn(...args)).then(
        (result) => {
          if (mountedRef.current) {
            setData(result)
            setIsLoading(false)
          }
          return result
        },
        (err) => {
          if (mountedRef.current) {
            setError(err)
            setIsLoading(false)
          }
          throw err
        },
      )
    },
    [effectFn],
  )

  return { mutate, data, error, isLoading }
}
