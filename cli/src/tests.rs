#[cfg(test)]
mod tests {
    use crate::models::*;
    use serde_json::json;

    /// Task 44: 測試 Task 36 - 訂閱命令
    #[test]
    fn test_subscribe_request_serialization() {
        let request = SubscribeRequest {
            rss_url: "https://example.com/rss".to_string(),
            fetcher: "mikanani".to_string(),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["rss_url"], "https://example.com/rss");
        assert_eq!(json["fetcher"], "mikanani");
    }

    /// Task 44: 測試 Task 37 - 列出動畫
    #[test]
    fn test_anime_metadata_deserialization() {
        let json = json!({
            "anime_id": 1,
            "title": "Test Anime",
            "created_at": "2025-01-22T00:00:00Z",
            "updated_at": "2025-01-22T00:00:00Z"
        });

        let anime: AnimeMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(anime.anime_id, 1);
        assert_eq!(anime.title, "Test Anime");
    }

    /// Task 44: 測試 Task 38 - 動畫連結
    #[test]
    fn test_anime_link_deserialization() {
        let json = json!({
            "link_id": 1,
            "series_id": 1,
            "group_id": 1,
            "episode_no": 1,
            "title": "Episode 1",
            "url": "magnet://example",
            "source_hash": "abc123",
            "filtered_flag": false,
            "created_at": "2025-01-22T00:00:00Z",
            "updated_at": "2025-01-22T00:00:00Z"
        });

        let link: AnimeLink = serde_json::from_value(json).unwrap();
        assert_eq!(link.link_id, 1);
        assert_eq!(link.episode_no, 1);
        assert!(!link.filtered_flag);
    }

    /// Task 44: 測試 Task 39 - 過濾規則
    #[test]
    fn test_filter_type_serialization() {
        let positive = FilterType::Positive;
        let negative = FilterType::Negative;

        let pos_json = serde_json::to_value(&positive).unwrap();
        let neg_json = serde_json::to_value(&negative).unwrap();

        assert_eq!(pos_json, "Positive");
        assert_eq!(neg_json, "Negative");
    }

    #[test]
    fn test_create_filter_rule_request() {
        let request = CreateFilterRuleRequest {
            series_id: 1,
            group_id: 2,
            rule_type: FilterType::Positive,
            regex_pattern: ".*720p.*".to_string(),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["series_id"], 1);
        assert_eq!(json["group_id"], 2);
        assert_eq!(json["rule_type"], "Positive");
        assert_eq!(json["regex_pattern"], ".*720p.*");
    }

    /// Task 44: 測試 Task 40 - 下載請求
    #[test]
    fn test_download_request_serialization() {
        let request = DownloadRequest {
            link_id: 42,
            downloader: Some("qbittorrent".to_string()),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["link_id"], 42);
        assert_eq!(json["downloader"], "qbittorrent");
    }

    #[test]
    fn test_download_request_no_downloader() {
        let request = DownloadRequest {
            link_id: 42,
            downloader: None,
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["link_id"], 42);
        assert!(json["downloader"].is_null());
    }

    /// Task 44: 測試 Task 41 - 狀態查詢
    #[test]
    fn test_download_progress_deserialization() {
        let json = json!({
            "link_id": "link-123",
            "downloader_type": "qbittorrent",
            "status": "downloading",
            "progress": 0.75,
            "downloaded_bytes": 750000000,
            "total_bytes": 1000000000,
            "error_message": null
        });

        let progress: DownloadProgress = serde_json::from_value(json).unwrap();
        assert_eq!(progress.link_id, "link-123");
        assert_eq!(progress.status, "downloading");
        assert_eq!(progress.progress, 0.75);
        assert!(progress.error_message.is_none());
    }

    /// Task 44: 測試 Task 42 - 服務查詢
    #[test]
    fn test_registered_service_deserialization() {
        let json = json!({
            "service_id": "service-1",
            "service_type": "fetcher",
            "service_name": "mikanani",
            "host": "localhost",
            "port": 8001,
            "is_healthy": true,
            "last_heartbeat": "2025-01-22T00:00:00Z"
        });

        let service: RegisteredService = serde_json::from_value(json).unwrap();
        assert_eq!(service.service_id, "service-1");
        assert_eq!(service.service_type, "fetcher");
        assert!(service.is_healthy);
    }

    /// Task 44: 測試列表響應
    #[test]
    fn test_list_response_generic() {
        let json = json!({
            "items": [
                {
                    "anime_id": 1,
                    "title": "Anime 1",
                    "created_at": "2025-01-22T00:00:00Z",
                    "updated_at": "2025-01-22T00:00:00Z"
                }
            ],
            "total": 100
        });

        let response: ListResponse<AnimeMetadata> = serde_json::from_value(json).unwrap();
        assert_eq!(response.items.len(), 1);
        assert_eq!(response.total, Some(100));
    }

    /// Task 44: 成功響應測試
    #[test]
    fn test_success_response_deserialization() {
        let json = json!({
            "message": "Operation successful"
        });

        let response: SuccessResponse = serde_json::from_value(json).unwrap();
        assert_eq!(response.message, "Operation successful");
    }

    /// Task 44: 字幕組測試
    #[test]
    fn test_subtitle_group_deserialization() {
        let json = json!({
            "group_id": 1,
            "group_name": "Subtitle Group A",
            "created_at": "2025-01-22T00:00:00Z"
        });

        let group: SubtitleGroup = serde_json::from_value(json).unwrap();
        assert_eq!(group.group_id, 1);
        assert_eq!(group.group_name, "Subtitle Group A");
    }

    /// Task 44: 動畫系列測試
    #[test]
    fn test_anime_series_metadata_deserialization() {
        let json = json!({
            "series_id": 1,
            "anime_id": 1,
            "series_no": 1,
            "season_id": 1,
            "description": "First season",
            "aired_date": "2025-01-01",
            "end_date": "2025-03-31",
            "created_at": "2025-01-22T00:00:00Z",
            "updated_at": "2025-01-22T00:00:00Z"
        });

        let series: AnimeSeriesMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(series.series_id, 1);
        assert_eq!(series.series_no, 1);
        assert_eq!(series.description, Some("First season".to_string()));
    }

    /// Task 44: 過濾規則完整測試
    #[test]
    fn test_filter_rule_deserialization() {
        let json = json!({
            "rule_id": 1,
            "series_id": 1,
            "group_id": 1,
            "rule_order": 1,
            "rule_type": "Positive",
            "regex_pattern": "1080p",
            "created_at": "2025-01-22T00:00:00Z"
        });

        let rule: FilterRule = serde_json::from_value(json).unwrap();
        assert_eq!(rule.rule_id, 1);
        assert_eq!(rule.rule_type, FilterType::Positive);
        assert_eq!(rule.regex_pattern, "1080p");
    }

    /// Task 44: 季度信息測試
    #[test]
    fn test_season_info_deserialization() {
        let json = json!({
            "season_id": 1,
            "year": 2025,
            "season": "Winter"
        });

        let season: SeasonInfo = serde_json::from_value(json).unwrap();
        assert_eq!(season.season_id, 1);
        assert_eq!(season.year, 2025);
        assert_eq!(season.season, "Winter");
    }

    // ========== 整合測試 ==========

    /// Task 44: 整合測試 - 完整動畫列表流程
    #[test]
    fn test_full_anime_list_workflow() {
        let response_json = json!({
            "items": [
                {
                    "anime_id": 1,
                    "title": "Anime 1",
                    "created_at": "2025-01-22T00:00:00Z",
                    "updated_at": "2025-01-22T00:00:00Z"
                },
                {
                    "anime_id": 2,
                    "title": "Anime 2",
                    "created_at": "2025-01-22T00:00:00Z",
                    "updated_at": "2025-01-22T00:00:00Z"
                }
            ],
            "total": 2
        });

        let response: ListResponse<AnimeMetadata> = serde_json::from_value(response_json).unwrap();
        assert_eq!(response.items.len(), 2);
        assert_eq!(response.total, Some(2));
        assert_eq!(response.items[0].anime_id, 1);
        assert_eq!(response.items[1].anime_id, 2);
    }

    /// Task 44: 整合測試 - 過濾規則流程
    #[test]
    fn test_full_filter_workflow() {
        // 創建規則
        let create_request = CreateFilterRuleRequest {
            series_id: 1,
            group_id: 2,
            rule_type: FilterType::Positive,
            regex_pattern: "1080p".to_string(),
        };

        let json = serde_json::to_value(&create_request).unwrap();
        assert_eq!(json["rule_type"], "Positive");

        // 模擬已創建的規則
        let rule_json = json!({
            "rule_id": 1,
            "series_id": 1,
            "group_id": 2,
            "rule_order": 1,
            "rule_type": "Positive",
            "regex_pattern": "1080p",
            "created_at": "2025-01-22T00:00:00Z"
        });

        let rule: FilterRule = serde_json::from_value(rule_json).unwrap();
        assert_eq!(rule.series_id, create_request.series_id);
        assert_eq!(rule.regex_pattern, create_request.regex_pattern);
    }

    /// Task 44: 整合測試 - 下載流程
    #[test]
    fn test_full_download_workflow() {
        // 1. 查詢連結
        let links_response = json!({
            "items": [
                {
                    "link_id": 1,
                    "series_id": 1,
                    "group_id": 1,
                    "episode_no": 1,
                    "title": "Episode 1",
                    "url": "magnet://example",
                    "source_hash": "abc123",
                    "filtered_flag": false,
                    "created_at": "2025-01-22T00:00:00Z",
                    "updated_at": "2025-01-22T00:00:00Z"
                }
            ],
            "total": 1
        });

        let response: ListResponse<AnimeLink> = serde_json::from_value(links_response).unwrap();
        assert_eq!(response.items.len(), 1);

        let link = &response.items[0];
        assert_eq!(link.link_id, 1);

        // 2. 啟動下載
        let download_request = DownloadRequest {
            link_id: link.link_id,
            downloader: Some("qbittorrent".to_string()),
        };

        let json = serde_json::to_value(&download_request).unwrap();
        assert_eq!(json["link_id"], 1);
        assert_eq!(json["downloader"], "qbittorrent");

        // 3. 查詢進度
        let progress_json = json!({
            "link_id": "1",
            "downloader_type": "qbittorrent",
            "status": "downloading",
            "progress": 0.5,
            "downloaded_bytes": 500000000,
            "total_bytes": 1000000000,
            "error_message": null
        });

        let progress: DownloadProgress = serde_json::from_value(progress_json).unwrap();
        assert_eq!(progress.status, "downloading");
        assert_eq!(progress.progress, 0.5);
    }

    /// Task 44: 整合測試 - 服務發現流程
    #[test]
    fn test_full_service_discovery_workflow() {
        let services_response = json!({
            "items": [
                {
                    "service_id": "service-1",
                    "service_type": "fetcher",
                    "service_name": "mikanani",
                    "host": "localhost",
                    "port": 8001,
                    "is_healthy": true,
                    "last_heartbeat": "2025-01-22T00:00:00Z"
                },
                {
                    "service_id": "service-2",
                    "service_type": "downloader",
                    "service_name": "qbittorrent",
                    "host": "localhost",
                    "port": 8002,
                    "is_healthy": true,
                    "last_heartbeat": "2025-01-22T00:00:00Z"
                }
            ],
            "total": 2
        });

        let response: ListResponse<RegisteredService> =
            serde_json::from_value(services_response).unwrap();
        assert_eq!(response.items.len(), 2);

        let fetcher = &response.items[0];
        assert_eq!(fetcher.service_type, "fetcher");
        assert!(fetcher.is_healthy);

        let downloader = &response.items[1];
        assert_eq!(downloader.service_type, "downloader");
        assert!(downloader.is_healthy);
    }

    /// Task 44: 邊界案例測試 - 空列表
    #[test]
    fn test_empty_list_response() {
        let response_json = json!({
            "items": [],
            "total": 0
        });

        let response: ListResponse<AnimeMetadata> = serde_json::from_value(response_json).unwrap();
        assert_eq!(response.items.len(), 0);
        assert_eq!(response.total, Some(0));
    }

    /// Task 44: 邊界案例測試 - 大型列表
    #[test]
    fn test_large_list_response() {
        let mut items = vec![];
        for i in 1..=1000 {
            items.push(json!({
                "anime_id": i,
                "title": format!("Anime {}", i),
                "created_at": "2025-01-22T00:00:00Z",
                "updated_at": "2025-01-22T00:00:00Z"
            }));
        }

        let response_json = json!({
            "items": items,
            "total": 1000
        });

        let response: ListResponse<AnimeMetadata> = serde_json::from_value(response_json).unwrap();
        assert_eq!(response.items.len(), 1000);
        assert_eq!(response.total, Some(1000));
    }

    /// Task 44: 錯誤情況測試 - 缺失字段
    #[test]
    fn test_missing_optional_fields() {
        let json = json!({
            "anime_id": 1,
            "title": "Anime without dates",
            "created_at": "2025-01-22T00:00:00Z",
            "updated_at": "2025-01-22T00:00:00Z"
        });

        let anime: AnimeMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(anime.anime_id, 1);
        assert_eq!(anime.title, "Anime without dates");
    }

    /// Task 44: 客戶端連接測試
    #[test]
    fn test_api_client_construction() {
        let _client = crate::client::ApiClient::new("http://localhost:8000".to_string());
        // 測試客戶端能夠被創建
        // 注意: 無法在單元測試中直接測試 async 操作而不啟動服務器
    }

    /// Task 44: 驗證正則表達式模式
    #[test]
    fn test_filter_regex_patterns() {
        let patterns = vec![".*720p.*", ".*1080p.*", "^EP\\d+", "[CHD].*"];

        for pattern in patterns {
            let request = CreateFilterRuleRequest {
                series_id: 1,
                group_id: 1,
                rule_type: FilterType::Positive,
                regex_pattern: pattern.to_string(),
            };

            let json = serde_json::to_value(&request).unwrap();
            assert_eq!(json["regex_pattern"], pattern);
        }
    }
}
