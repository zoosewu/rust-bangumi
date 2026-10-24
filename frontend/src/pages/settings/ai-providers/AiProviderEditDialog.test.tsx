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
        onListModels={vi.fn().mockResolvedValue([])}
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

  it("lists models with the saved key when editing and fills the chosen one", async () => {
    const onListModels = vi.fn().mockResolvedValue(["llama-3.3-70b", "openai/gpt-oss-120b"])
    render(
      <AiProviderEditDialog
        provider={{
          id: 7,
          name: "Groq",
          provider_kind: "openai_compatible",
          base_url: "https://api.groq.com/openai/v1",
          api_key: "••••••••",
          model_name: "",
          max_tokens: 4096,
          response_format_mode: "strict",
          is_enabled: true,
          priority: 0,
          created_at: "",
          updated_at: "",
          timeout_secs: 60,
          parser_prompt: null,
          filter_prompt: null,
        }}
        onClose={vi.fn()}
        onSubmit={vi.fn()}
        onTestConfig={vi.fn()}
        onListModels={onListModels}
      />,
    )

    const trigger = screen.getByRole("button", { name: "選擇模型" })
    fireEvent.keyDown(trigger, { key: "Enter" })

    await waitFor(() =>
      expect(onListModels).toHaveBeenCalledWith({
        existing_provider_id: 7,
        base_url: "https://api.groq.com/openai/v1",
        api_key: "",
      }),
    )
    fireEvent.click(await screen.findByText("openai/gpt-oss-120b"))
    expect(screen.getByPlaceholderText("gpt-4o-mini")).toHaveValue("openai/gpt-oss-120b")
  })

  it("shows why the model list could not be loaded", async () => {
    render(
      <AiProviderEditDialog
        provider={null}
        onClose={vi.fn()}
        onSubmit={vi.fn()}
        onTestConfig={vi.fn()}
        onListModels={vi.fn().mockRejectedValue(new Error("provider 回應 HTTP 401"))}
      />,
    )
    fireEvent.change(screen.getByPlaceholderText("https://api.openai.com/v1"), {
      target: { value: "https://api.example.test/v1" },
    })
    fireEvent.keyDown(screen.getByRole("button", { name: "選擇模型" }), { key: "Enter" })
    expect(await screen.findByText("provider 回應 HTTP 401")).toBeInTheDocument()
  })

  it("saves the timeout and the model prompts, and blocks an out-of-range timeout", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined)
    render(
      <AiProviderEditDialog
        provider={{
          id: 3,
          name: "mistral",
          provider_kind: "openai_compatible",
          base_url: "https://api.mistral.ai/v1",
          api_key: "••••••••",
          model_name: "ministral-14b-latest",
          max_tokens: 4096,
          response_format_mode: "strict",
          is_enabled: true,
          priority: 2,
          created_at: "",
          updated_at: "",
          timeout_secs: 120,
          parser_prompt: "long parser prompt",
          filter_prompt: null,
        }}
        onClose={vi.fn()}
        onSubmit={onSubmit}
        onTestConfig={vi.fn()}
        onListModels={vi.fn().mockResolvedValue([])}
      />,
    )

    const timeout = screen.getByDisplayValue("120")
    fireEvent.change(timeout, { target: { value: "3" } })
    expect(screen.getByRole("button", { name: "儲存" })).toBeDisabled()

    fireEvent.change(timeout, { target: { value: "90" } })
    fireEvent.change(screen.getByDisplayValue("long parser prompt"), {
      target: { value: "" },
    })
    fireEvent.change(screen.getByPlaceholderText("留空：使用全域的過濾固定 prompt"), {
      target: { value: "short filter prompt" },
    })
    fireEvent.click(screen.getByRole("button", { name: "儲存" }))

    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith(
        expect.objectContaining({
          timeout_secs: 90,
          parser_prompt: "",
          filter_prompt: "short filter prompt",
        }),
      ),
    )
  })

})
