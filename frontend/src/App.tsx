import { BrowserRouter, Routes, Route } from "react-router-dom"
import { AppLayout } from "@/components/layout/AppLayout"
import { Toaster } from "@/components/ui/sonner"
import Dashboard from "@/pages/Dashboard"
import AnimePage from "@/pages/anime-series/AnimeSeriesPage"
import AnimeWorksPage from "@/pages/anime/AnimePage"
import SubscriptionsPage from "@/pages/subscriptions/SubscriptionsPage"
import RawItemsPage from "@/pages/raw-items/RawItemsPage"
import ConflictsPage from "@/pages/conflicts/ConflictsPage"
import SubtitleGroupsPage from "@/pages/subtitle-groups/SubtitleGroupsPage"
import ParsersPage from "@/pages/parsers/ParsersPage"

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route index element={<Dashboard />} />
          <Route path="anime" element={<AnimePage />} />
          <Route path="subscriptions" element={<SubscriptionsPage />} />
          <Route path="raw-items" element={<RawItemsPage />} />
          <Route path="conflicts" element={<ConflictsPage />} />
          <Route path="anime-works" element={<AnimeWorksPage />} />
          <Route path="subtitle-groups" element={<SubtitleGroupsPage />} />
          <Route path="parsers" element={<ParsersPage />} />
        </Route>
      </Routes>
      <Toaster />
    </BrowserRouter>
  )
}
