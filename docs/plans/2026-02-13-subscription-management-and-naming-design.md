# Subscription Management & Naming Adjustments Design

**Date:** 2026-02-13

## Overview

Three changes to the frontend (and minimal backend additions):

1. **Rename UI labels**: "動畫季度" → "動畫", "動畫" → "動畫作品"
2. **Add subscription creation**: New subscription dialog with source_url, name, interval
3. **Add subscription deletion**: With cascade delete of pending/failed raw items and confirmation dialog

## 1. Naming Adjustments

### Translation Changes (all 3 languages)

| Key | EN (before → after) | zh-TW (before → after) | ja (before → after) |
|-----|---------------------|------------------------|---------------------|
| `sidebar.animeSeries` | Anime Seasons → Anime | 動畫季度 → 動畫 | アニメシーズン → アニメ |
| `sidebar.anime` | Anime → Anime Titles | 動畫 → 動畫作品 | アニメ → アニメ作品 |
| `animeSeries.title` | Anime Seasons → Anime | 動畫季度 → 動畫 | アニメシーズン → アニメ |
| `anime.title` | Anime → Anime Titles | 動畫 → 動畫作品 | アニメ → アニメ作品 |
| `anime.addAnime` | Add Anime → Add Anime Title | 新增動畫 → 新增動畫作品 | アニメ追加 → アニメ作品追加 |
| `anime.animeTitle` | Anime title → Anime title name | (keep) | (keep) |
| `anime.deleteAnime` | Delete Anime → Delete Anime Title | 刪除動畫 → 刪除動畫作品 | アニメを削除 → アニメ作品を削除 |
| `anime.notFound` | Anime not found → Anime title not found | 找不到動畫 → 找不到動畫作品 | アニメが見つかりません → アニメ作品が見つかりません |
| `anime.noRules` | ...for this anime → ...for this anime title | 此動畫沒有... → 此動畫作品沒有... | このアニメには... → このアニメ作品には... |
| `dashboard.totalAnime` | Anime → Anime Titles | 動畫 → 動畫作品 | アニメ → アニメ作品 |
| `dashboard.totalSeries` | Seasons → Anime | 季度 → 動畫 | シーズン → アニメ |

Note: `animeSeries.animeTitle` (label for the anime field inside a series) stays as "Anime" / "動畫" / "アニメ" since it refers to the parent anime title entity.

## 2. Subscription Creation

### Frontend

- Add "Add Subscription" button to SubscriptionsPage header (consistent with AnimePage pattern)
- Dialog with fields:
  - `source_url` (required) — RSS feed URL
  - `name` (optional) — display name
  - `fetch_interval_minutes` (required, default: 30) — fetch interval in minutes
- On submit: call existing `POST /subscriptions` API
- Backend auto-selects fetcher and triggers first fetch

### New i18n Keys

```
subscriptions.addSubscription
subscriptions.deleteSubscription
subscriptions.deleteConfirm
subscriptions.affectedRawItems
```

### Frontend API Addition

- `CoreApi.createSubscription(req)` — calls `POST /api/core/subscriptions`
- `CoreApi.deleteSubscription(id)` — calls `DELETE /api/core/subscriptions/:id`
- `CoreApi.getRawItemsCount(subscriptionId, statuses)` — calls `GET /api/core/raw-items/count`

## 3. Subscription Deletion

### Flow

1. User clicks delete button on subscription row
2. Frontend calls `GET /raw-items/count?subscription_id=X&status=pending,failed`
3. ConfirmDialog shows: "確定要刪除此訂閱嗎？將同時刪除 N 筆未完成的原始項目。"
4. On confirm: calls `DELETE /subscriptions/:id` (backend handles cascade)

### Backend Changes

1. **New endpoint**: `GET /raw-items/count?subscription_id=X&status=pending,failed`
   - Returns `{ "count": N }`

2. **Modify `DELETE /subscriptions`**: Change path param from `:rss_url` to `:id` (subscription_id)
   - Before deleting subscription, delete raw_anime_items where subscription_id = X AND status IN ('pending', 'failed')
   - Return count of deleted raw items in response

## Non-Goals

- No changes to backend subscription creation logic
- No changes to anime/animeSeries page functionality (only labels)
- No changes to raw items page
