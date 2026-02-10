import { ManagedRuntime, Layer } from "effect"
import * as BrowserHttpClient from "@effect/platform-browser/BrowserHttpClient"
import { CoreApiLive } from "@/layers/ApiLayer"

const AppLayer = CoreApiLive.pipe(
  Layer.provide(BrowserHttpClient.layerXMLHttpRequest),
)

export const AppRuntime = ManagedRuntime.make(AppLayer)
