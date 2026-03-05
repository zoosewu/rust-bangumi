import { BrowserRouter, Routes, Route } from "react-router-dom"
import { AppLayout } from "@/components/layout/AppLayout"
import { Toaster } from "@/components/ui/sonner"
import Dashboard from "@/pages/Dashboard"
import AnimePage from "@/pages/anime-series/AnimeSeriesPage"
import AnimeWorksPage from "@/pages/anime/AnimePage"
import SubscriptionsPage from "@/pages/subscriptions/SubscriptionsPage"
import RawItemsPage from "@/pages/raw-items/RawItemsPage"
import SubtitleGroupsPage from "@/pages/subtitle-groups/SubtitleGroupsPage"
import ParsersPage from "@/pages/parsers/ParsersPage"
import FiltersPage from "@/pages/filters/FiltersPage"
import SearchPage from "@/pages/search/SearchPage"
import PendingPage from "@/pages/pending/PendingPage"
import SettingsPage from "@/pages/settings/SettingsPage"

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route index element={<Dashboard />} />
          <Route path="anime" element={<AnimePage />} />
          <Route path="subscriptions" element={<SubscriptionsPage />} />
          <Route path="raw-items" element={<RawItemsPage />} />
          <Route path="pending" element={<PendingPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="anime-works" element={<AnimeWorksPage />} />
          <Route path="subtitle-groups" element={<SubtitleGroupsPage />} />
          <Route path="parsers" element={<ParsersPage />} />
          <Route path="filters" element={<FiltersPage />} />
          <Route path="search" element={<SearchPage />} />
        </Route>
      </Routes>
      <Toaster />
    </BrowserRouter>
  )
}
