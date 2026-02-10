import { BrowserRouter, Routes, Route } from "react-router-dom"
import { AppLayout } from "@/components/layout/AppLayout"
import Dashboard from "@/pages/Dashboard"
import AnimePage from "@/pages/anime/AnimePage"
import AnimeDetailPage from "@/pages/anime/AnimeDetailPage"
import SubscriptionsPage from "@/pages/subscriptions/SubscriptionsPage"
import RawItemsPage from "@/pages/raw-items/RawItemsPage"

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
        </Route>
      </Routes>
    </BrowserRouter>
  )
}
