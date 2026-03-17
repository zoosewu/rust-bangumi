import { describe, it, expect, vi, beforeEach } from "vitest"
import { renderHook, act, waitFor } from "@testing-library/react"
import { Effect } from "effect"

// Mock AppRuntime before importing the hook
const runPromiseMock = vi.fn()

vi.mock("@/runtime/AppRuntime", () => ({
  AppRuntime: {
    runPromise: runPromiseMock,
  },
}))

// Import hook after mock is set up
const { useEffectMutation } = await import("./useEffectMutation")

describe("useEffectMutation", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it("starts in idle state: isLoading=false, data=null, error=null", () => {
    const { result } = renderHook(() =>
      useEffectMutation((_arg: string) => Effect.succeed("ok")),
    )

    expect(result.current.isLoading).toBe(false)
    expect(result.current.data).toBeNull()
    expect(result.current.error).toBeNull()
  })

  it("sets isLoading=true while mutation is pending", async () => {
    let resolve: (v: string) => void
    const pending = new Promise<string>((r) => {
      resolve = r
    })
    runPromiseMock.mockReturnValue(pending)

    const { result } = renderHook(() =>
      useEffectMutation((_arg: string) => Effect.succeed("value")),
    )

    act(() => {
      result.current.mutate("arg")
    })

    expect(result.current.isLoading).toBe(true)

    // Resolve so the hook can clean up
    await act(async () => {
      resolve!("value")
      await pending
    })
  })

  it("sets data and clears isLoading on success", async () => {
    runPromiseMock.mockResolvedValue(42)

    const { result } = renderHook(() =>
      useEffectMutation((_x: number) => Effect.succeed(42)),
    )

    await act(async () => {
      await result.current.mutate(1)
    })

    expect(result.current.isLoading).toBe(false)
    expect(result.current.data).toBe(42)
    expect(result.current.error).toBeNull()
  })

  it("returns the result value from mutate()", async () => {
    runPromiseMock.mockResolvedValue("returned-value")

    const { result } = renderHook(() =>
      useEffectMutation((_arg: string) => Effect.succeed("returned-value")),
    )

    let returnedFromMutate: unknown
    await act(async () => {
      returnedFromMutate = await result.current.mutate("input")
    })

    expect(returnedFromMutate).toBe("returned-value")
  })

  it("sets error and clears isLoading on failure", async () => {
    const err = new Error("mutation failed")
    runPromiseMock.mockRejectedValue(err)

    const { result } = renderHook(() =>
      useEffectMutation((_arg: string) => Effect.fail(err)),
    )

    await act(async () => {
      try {
        await result.current.mutate("input")
      } catch {
        // expected to throw
      }
    })

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false)
    })

    expect(result.current.error).toBe(err)
    expect(result.current.data).toBeNull()
  })

  it("re-throws the error from mutate() on failure", async () => {
    const err = new Error("boom")
    runPromiseMock.mockRejectedValue(err)

    const { result } = renderHook(() =>
      useEffectMutation((_arg: string) => Effect.fail(err)),
    )

    await act(async () => {
      await expect(result.current.mutate("input")).rejects.toThrow("boom")
    })
  })

  it("handles multi-argument effectFn", async () => {
    runPromiseMock.mockResolvedValue("combined")

    const { result } = renderHook(() =>
      useEffectMutation((a: string, b: number) =>
        Effect.succeed(`${a}-${b}`),
      ),
    )

    await act(async () => {
      await result.current.mutate("hello", 3)
    })

    expect(result.current.data).toBe("combined")
  })
})
