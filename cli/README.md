# 動畫 RSS 聚合系統 - CLI 工具

## 概述

這是 Bangumi 動畫 RSS 聚合系統的命令行工具，提供完整的命令行操作界面來管理動畫訂閱、下載、過濾規則和系統狀態。

## 功能特性

- **RSS 訂閱管理**: 訂閱和管理動畫 RSS 源
- **動畫列表**: 查詢所有訂閱的動畫及詳細信息
- **連結管理**: 查看和管理動畫下載連結
- **過濾規則**: 添加、查看和刪除過濾規則
- **下載控制**: 手動啟動下載任務
- **系統監控**: 查看系統狀態和已註冊服務
- **日誌查詢**: 查看系統日誌

## 安裝

### 從源代碼構建

```bash
cd /nodejs/rust-bangumi
cargo build --release --package bangumi-cli
```

### 二進制位置

構建完成後，可執行文件位於：
```
target/release/bangumi-cli
```

## 使用方法

### 基本命令格式

```bash
bangumi-cli [OPTIONS] <COMMAND>
```

### 全局選項

```
--api-url <API_URL>
    API 服務器地址 (默認: http://localhost:8000)
```

## 命令詳解

### 1. 訂閱 RSS 源 (Task 36)

**命令**: `subscribe`

**功能**: 訂閱新的動畫 RSS 源

**用法**:
```bash
bangumi-cli subscribe <RSS_URL> --fetcher <FETCHER_NAME>
```

**參數**:
- `RSS_URL`: RSS 源的完整 URL
- `--fetcher`: 擷取器名稱（如: mikanani）

**示例**:
```bash
bangumi-cli subscribe "https://mikanani.me/rss/active" --fetcher mikanani
```

**輸出**:
```
✓ 訂閱成功
  RSS 地址: https://mikanani.me/rss/active
  擷取器: mikanani
```

---

### 2. 列出動畫 (Task 37)

**命令**: `list`

**功能**: 顯示所有訂閱的動畫或特定動畫的詳細信息

**用法**:
```bash
# 列出所有動畫
bangumi-cli list

# 獲取特定動畫信息
bangumi-cli list --anime-id <ID>
```

**參數**:
- `--anime-id`: （可選）特定動畫的 ID
- `--season`: （可選）季度過濾（格式: 2025/冬）

**示例**:
```bash
# 列出所有動畫
bangumi-cli list

# 查詢 ID 為 1 的動畫
bangumi-cli list --anime-id 1
```

**輸出**:
```
+--------+------------------+---------------------+
| 動畫 ID | 標題             | 建立時間            |
+--------+------------------+---------------------+
| 1      | Anime Title 1    | 2025-01-22T00:00... |
| 2      | Anime Title 2    | 2025-01-22T00:00... |
+--------+------------------+---------------------+

總計: 2 個動畫
```

---

### 3. 列出連結 (Task 38)

**命令**: `links`

**功能**: 查看特定動畫的所有下載連結

**用法**:
```bash
bangumi-cli links <ANIME_ID> [--series <SERIES_NO>] [--group <GROUP_NAME>]
```

**參數**:
- `ANIME_ID`: 動畫 ID（必需）
- `--series`: （可選）季數過濾
- `--group`: （可選）字幕組過濾

**示例**:
```bash
# 列出動畫 ID 1 的所有連結
bangumi-cli links 1

# 列出第一季的連結
bangumi-cli links 1 --series 1

# 列出特定字幕組的連結
bangumi-cli links 1 --group "字幕組A"
```

**輸出**:
```
+---------+----+--------+----------+------------------+----+
| 連結 ID | 集 | 字幕組 | 標題     | URL              | 狀 |
+---------+----+--------+----------+------------------+----+
| 1       | 1  | 1      | Episode1 | magnet://...     | 活 |
| 2       | 2  | 1      | Episode2 | magnet://...     | 活 |
+---------+----+--------+----------+------------------+----+
```

---

### 4. 管理過濾規則 (Task 39)

#### 添加過濾規則

**命令**: `filter add`

**功能**: 添加新的過濾規則

**用法**:
```bash
bangumi-cli filter add <SERIES_ID> <GROUP_ID> <RULE_TYPE> <REGEX>
```

**參數**:
- `SERIES_ID`: 系列 ID
- `GROUP_ID`: 字幕組 ID
- `RULE_TYPE`: 規則類型 (positive/negative 或 正向/反向)
- `REGEX`: 正則表達式模式

**示例**:
```bash
# 添加正向過濾規則（只下載 1080p）
bangumi-cli filter add 1 1 positive ".*1080p.*"

# 添加反向過濾規則（排除 480p）
bangumi-cli filter add 1 1 negative ".*480p.*"
```

**輸出**:
```
✓ 過濾規則已添加
  系列 ID: 1
  字幕組 ID: 1
  規則類型: positive
  正則表達式: .*1080p.*
```

#### 列出過濾規則

**命令**: `filter list`

**功能**: 查看特定系列和字幕組的過濾規則

**用法**:
```bash
bangumi-cli filter list <SERIES_ID> <GROUP_ID>
```

**示例**:
```bash
bangumi-cli filter list 1 1
```

**輸出**:
```
+---------+----------+--------+------+-------------+------------------+
| 規則 ID | 系列 ID  | 字幕組 | 類型 | 正則表達式  | 建立時間         |
+---------+----------+--------+------+-------------+------------------+
| 1       | 1        | 1      | 正向 | .*1080p.*   | 2025-01-22T...   |
+---------+----------+--------+------+-------------+------------------+
```

#### 刪除過濾規則

**命令**: `filter remove`

**功能**: 刪除指定的過濾規則

**用法**:
```bash
bangumi-cli filter remove <RULE_ID>
```

**示例**:
```bash
bangumi-cli filter remove 1
```

**輸出**:
```
✓ 過濾規則已刪除
  規則 ID: 1
```

---

### 5. 啟動下載 (Task 40)

**命令**: `download`

**功能**: 手動啟動特定連結的下載

**用法**:
```bash
bangumi-cli download <LINK_ID> [--downloader <DOWNLOADER_NAME>]
```

**參數**:
- `LINK_ID`: 連結 ID（必需）
- `--downloader`: （可選）下載器名稱（如: qbittorrent）

**示例**:
```bash
# 使用默認下載器下載
bangumi-cli download 1

# 使用特定下載器下載
bangumi-cli download 1 --downloader qbittorrent
```

**輸出**:
```
✓ 下載已啟動
  連結 ID: 1
```

---

### 6. 查看狀態 (Task 41)

**命令**: `status`

**功能**: 查看系統整體狀態

**用法**:
```bash
bangumi-cli status
```

**示例**:
```bash
bangumi-cli status
```

**輸出**:
```
系統狀態:
{
  "status": "ok",
  "service": "core-service"
}
```

---

### 7. 列出服務 (Task 42)

**命令**: `services`

**功能**: 列出所有已註冊的服務

**用法**:
```bash
bangumi-cli services
```

**示例**:
```bash
bangumi-cli services
```

**輸出**:
```
+-----------+------+----------+----------+----+-----+------------------+
| 服務 ID   | 類型 | 服務名稱 | 主機     | 埠 | 狀態| 最後心跳         |
+-----------+------+----------+----------+----+-----+------------------+
| service-1 | 擷取 | mikanani | localhos | 80 | ✓   | 2025-01-22T...   |
| service-2 | 下載 | qbittore | localhos | 80 | ✓   | 2025-01-22T...   |
+-----------+------+----------+----------+----+-----+------------------+
```

---

### 8. 查看日誌 (Task 43)

**命令**: `logs`

**功能**: 查看系統日誌

**用法**:
```bash
bangumi-cli logs --type <LOG_TYPE>
```

**參數**:
- `--type`: 日誌類型 (cron | download)

**示例**:
```bash
# 查看 Cron 日誌
bangumi-cli logs --type cron

# 查看下載日誌
bangumi-cli logs --type download
```

**輸出**:
```
日誌查詢: cron
注意: 日誌功能需要在核心服務中實現日誌端點
```

---

## 環境變量

### CORE_SERVICE_URL

設置核心服務的 URL（可選，默認值: http://localhost:8000）

```bash
export CORE_SERVICE_URL=http://api.example.com:8000
bangumi-cli list
```

### RUST_LOG

控制日誌級別

```bash
export RUST_LOG=bangumi_cli=debug
bangumi-cli list
```

---

## 配置

### API 基礎 URL

通過 `--api-url` 全局選項指定 API 服務器地址：

```bash
bangumi-cli --api-url http://api.example.com:8000 list
```

或通過環境變量設置：

```bash
export CORE_SERVICE_URL=http://api.example.com:8000
```

---

## Docker 部署

### 構建 Docker 鏡像

```bash
cd /nodejs/rust-bangumi
docker build -f Dockerfile.cli -t bangumi-cli:latest .
```

### 運行 CLI 容器

```bash
docker run --rm \
  -e CORE_SERVICE_URL=http://core-service:8000 \
  bangumi-cli:latest \
  list
```

---

## 測試 (Task 44)

### 運行所有測試

```bash
cargo test --package bangumi-cli
```

### 運行特定測試

```bash
cargo test --package bangumi-cli test_subscribe_request_serialization
```

### 查看測試覆蓋率

```bash
# 安裝 tarpaulin (可選)
cargo install cargo-tarpaulin

# 生成覆蓋率報告
cargo tarpaulin --package bangumi-cli
```

### 測試統計

- **總測試數**: 24+ 個集成和單元測試
- **覆蓋範圍**:
  - ✓ 所有 8 個命令的參數序列化
  - ✓ 所有響應類型的反序列化
  - ✓ 完整的工作流程測試
  - ✓ 邊界案例測試
  - ✓ HTTP 客戶端功能

---

## HTTP 客戶端 (Task 35)

### 客戶端功能

CLI 使用自定義 HTTP 客戶端 (`ApiClient`) 來與核心服務通信。

### 主要方法

#### GET 請求

```rust
let response: ListResponse<AnimeMetadata> = client.get("/anime").await?;
```

#### POST 請求

```rust
let request = SubscribeRequest {
    rss_url: "...".to_string(),
    fetcher: "mikanani".to_string(),
};
let response: SuccessResponse = client.post("/anime", &request).await?;
```

#### DELETE 請求

```rust
client.delete("/filters/1").await?;
```

### 錯誤處理

客戶端自動處理 HTTP 錯誤，並返回 `anyhow::Result<T>`：

```rust
match client.get::<AnimeMetadata>("/anime/999").await {
    Ok(anime) => println!("Found: {:?}", anime),
    Err(e) => eprintln!("Error: {}", e),
}
```

---

## 常見用例

### 用例 1: 訂閱新 RSS 源

```bash
# 訂閱 Mikanani 的 RSS
bangumi-cli subscribe "https://mikanani.me/rss/active" --fetcher mikanani

# 列出所有動畫
bangumi-cli list

# 檢查連結
bangumi-cli links 1
```

### 用例 2: 設置過濾規則

```bash
# 為系列 1 的字幕組 1 添加過濾規則
# 只下載 1080p 版本
bangumi-cli filter add 1 1 positive ".*1080p.*"

# 排除低分辨率版本
bangumi-cli filter add 1 1 negative ".*480p.*"

# 查看所有規則
bangumi-cli filter list 1 1
```

### 用例 3: 手動下載

```bash
# 列出可用的連結
bangumi-cli links 1

# 手動下載特定連結
bangumi-cli download 5 --downloader qbittorrent

# 檢查系統狀態
bangumi-cli status
```

### 用例 4: 系統監控

```bash
# 列出所有服務
bangumi-cli services

# 查看系統狀態
bangumi-cli status

# 查看日誌
bangumi-cli logs --type cron
```

---

## 故障排除

### 連接超時

**症狀**: `TCP connection timeout`

**解決方案**:
1. 確認 API 服務器正在運行
2. 驗證 `--api-url` 或 `CORE_SERVICE_URL` 設置正確
3. 檢查防火牆設置

```bash
# 測試連接
curl http://localhost:8000/health
```

### 無效的 API 響應

**症狀**: `Failed to parse response`

**解決方案**:
1. 確認 API 版本兼容
2. 查看詳細日誌

```bash
export RUST_LOG=bangumi_cli=debug,reqwest=debug
bangumi-cli list
```

### 授權錯誤

**症狀**: `HTTP 401: Unauthorized`

**解決方案**:
1. 驗證 API 認證令牌（如果需要）
2. 檢查 API 文檔

---

## 依賴項

### 運行時依賴

- `tokio`: 異步運行時
- `reqwest`: HTTP 客戶端
- `serde`: 序列化/反序列化
- `clap`: 命令行參數解析
- `tracing`: 日誌記錄
- `prettytable-rs`: 表格格式化

### 開發依賴

- `tokio`: 測試 async 代碼
- `serde_json`: JSON 測試

---

## 性能考慮

### 並發請求

CLI 支持通過異步 I/O 進行高效的網絡操作：

```bash
# 即使有許多動畫，列表命令仍然響應迅速
bangumi-cli list
```

### 連接復用

HTTP 客戶端自動復用連接以提高效率

### 內存使用

- 列表響應流式處理以最小化內存使用
- 大型數據集分頁支持

---

## API 端點映射

| CLI 命令 | HTTP 方法 | 端點 | Task |
|---------|----------|------|------|
| subscribe | POST | /anime | 36 |
| list | GET | /anime[/{id}] | 37 |
| links | GET | /links/{id} | 38 |
| filter add | POST | /filters | 39 |
| filter list | GET | /filters/{series}/{group} | 39 |
| filter remove | DELETE | /filters/{id} | 39 |
| download | POST | /download | 40 |
| status | GET | /health | 41 |
| services | GET | /services | 42 |
| logs | - | - | 43 |

---

## 開發者指南

### 添加新命令

1. 在 `cli/src/main.rs` 的 `Commands` 枚舉中添加新命令
2. 在 `cli/src/commands.rs` 中實現命令處理函數
3. 添加測試用例到 `cli/src/tests.rs`
4. 更新文檔

### 擴展 HTTP 客戶端

編輯 `cli/src/client.rs` 中的 `ApiClient` 結構體：

```rust
impl ApiClient {
    pub async fn patch<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> anyhow::Result<R> {
        // 實現 PATCH 方法
    }
}
```

---

## 許可證

MIT

---

## 相關文檔

- [項目主 README](../README.md)
- [開發指南](../DEVELOPMENT.md)
- [架構設計](../docs/plans/2025-01-21-rust-bangumi-architecture-design.md)
- [實現計劃](../docs/plans/2025-01-21-implementation-plan.md)

---

**最後更新**: 2025-01-22
**版本**: 0.1.0
**狀態**: Phase 9 完成 ✓
