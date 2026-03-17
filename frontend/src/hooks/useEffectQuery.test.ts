import { describe, it, expect, vi, beforeEach } from "vitest"
import { renderHook, waitFor } from "@testing-library/react"
import { Effect } from "effect"

// Mock AppRuntime before importing the hook
const runPromiseMock = vi.fn()

vi.mock("@/runtime/AppRuntime", () => ({
  AppRuntime: {
    runPromise: runPromiseMock,
  },
}))

// Import hook after mock is set up
const { useEffectQuery } = await import("./useEffectQuery")

describe("useEffectQuery", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it("starts with isLoading=true and data=null", () => {
    // Return a promise that never resolves during this check
    runPromiseMock.mockReturnValue(new Promise(() => {}))

    const { result } = renderHook(() =>
      useEffectQuery(() => Effect.succeed("data")),
    )

    expect(result.current.isLoading).toBe(true)
    expect(result.current.data).toBeNull()
    expect(result.current.error).toBeNull()
  })

  it("sets data and clears isLoading on success", async () => {
    runPromiseMock.mockResolvedValue("hello")

    const { result } = renderHook(() =>
      useEffectQuery(() => Effect.succeed("hello")),
    )

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false)
    })

    expect(result.current.data).toBe("hello")
    expect(result.current.error).toBeNull()
  })

  it("sets error and clears isLoading on failure", async () => {
    const err = new Error("fetch failed")
    runPromiseMock.mockRejectedValue(err)

    const { result } = renderHook(() =>
      useEffectQuery(() => Effect.fail(err)),
    )

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false)
    })

    expect(result.current.error).toBe(err)
    expect(result.current.data).toBeNull()
  })

  it("returns a refetch function that re-runs the effect", async () => {
    runPromiseMock
      .mockResolvedValueOnce("first")
      .mockResolvedValueOnce("second")

    const { result } = renderHook(() =>
      useEffectQuery(() => Effect.succeed("value")),
    )

    await waitFor(() => {
      expect(result.current.data).toBe("first")
    })

    result.current.refetch()

    await waitFor(() => {
      expect(result.current.data).toBe("second")
    })

    expect(runPromiseMock).toHaveBeenCalledTimes(2)
  })

  it("passes result of array type correctly", async () => {
    const items = [{ id: 1 }, { id: 2 }]
    runPromiseMock.mockResolvedValue(items)

    const { result } = renderHook(() =>
      useEffectQuery(() => Effect.succeed(items)),
    )

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false)
    })

    expect(result.current.data).toEqual(items)
  })
})
