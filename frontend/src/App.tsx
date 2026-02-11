import { BrowserRouter, Routes, Route } from "react-router-dom"
import { AppLayout } from "@/components/layout/AppLayout"
import { Toaster } from "@/components/ui/sonner"
import Dashboard from "@/pages/Dashboard"
import AnimePage from "@/pages/anime/AnimePage"
import AnimeDetailPage from "@/pages/anime/AnimeDetailPage"
import SubscriptionsPage from "@/pages/subscriptions/SubscriptionsPage"
import RawItemsPage from "@/pages/raw-items/RawItemsPage"
import FiltersPage from "@/pages/filters/FiltersPage"
import ParsersPage from "@/pages/parsers/ParsersPage"
import DownloadsPage from "@/pages/downloads/DownloadsPage"
import ConflictsPage from "@/pages/conflicts/ConflictsPage"
import SubtitleGroupsPage from "@/pages/subtitle-groups/SubtitleGroupsPage"
import SubtitleGroupDetailPage from "@/pages/subtitle-groups/SubtitleGroupDetailPage"
import AnimeSeriesDetailPage from "@/pages/anime-series/AnimeSeriesDetailPage"

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route index element={<Dashboard />} />
          <Route path="anime" element={<AnimePage />} />
          <Route path="anime/:animeId" element={<AnimeDetailPage />} />
          <Route path="subscriptions" element={<SubscriptionsPage />} />
          <Route path="raw-items" element={<RawItemsPage />} />
          <Route path="filters" element={<FiltersPage />} />
          <Route path="parsers" element={<ParsersPage />} />
          <Route path="downloads" element={<DownloadsPage />} />
          <Route path="conflicts" element={<ConflictsPage />} />
          <Route path="subtitle-groups" element={<SubtitleGroupsPage />} />
          <Route path="subtitle-groups/:groupId" element={<SubtitleGroupDetailPage />} />
          <Route path="anime-series/:seriesId" element={<AnimeSeriesDetailPage />} />
        </Route>
      </Routes>
      <Toaster />
    </BrowserRouter>
  )
}
