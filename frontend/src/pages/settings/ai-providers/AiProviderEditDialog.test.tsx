import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"
import { AiProviderEditDialog } from "./AiProviderEditDialog"
import type { CreateAiProviderRequest } from "@/schemas/ai"

describe("AiProviderEditDialog", () => {
  it("tests the current form values without saving first", async () => {
    const onSubmit = vi.fn()
    const onTestConfig = vi.fn().mockResolvedValue({ ok: true })

    render(
      <AiProviderEditDialog
        provider={null}
        onClose={vi.fn()}
        onSubmit={onSubmit}
        onTestConfig={onTestConfig}
      />,
    )

    fireEvent.change(screen.getByPlaceholderText("https://api.openai.com/v1"), {
      target: { value: "https://api.example.test/v1" },
    })
    fireEvent.change(screen.getByPlaceholderText("sk-..."), {
      target: { value: "test-key" },
    })
    fireEvent.change(screen.getByPlaceholderText("gpt-4o-mini"), {
      target: { value: "gpt-test" },
    })

    fireEvent.click(screen.getByRole("button", { name: "測試" }))

    await waitFor(() => {
      expect(onTestConfig).toHaveBeenCalledWith(
        expect.objectContaining<CreateAiProviderRequest>({
          name: "",
          provider_kind: "openai_compatible",
          base_url: "https://api.example.test/v1",
          api_key: "test-key",
          model_name: "gpt-test",
          max_tokens: 4096,
          response_format_mode: "non_strict",
          is_enabled: true,
        }),
      )
    })
    expect(onSubmit).not.toHaveBeenCalled()
  })
})
