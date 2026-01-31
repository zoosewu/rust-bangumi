-- 建立 parser_source_type ENUM
CREATE TYPE parser_source_type AS ENUM ('regex', 'static');

-- 建立 title_parsers 表
CREATE TABLE title_parsers (
    parser_id               SERIAL PRIMARY KEY,
    name                    VARCHAR(100) NOT NULL,
    description             TEXT,
    priority                INT NOT NULL DEFAULT 0,
    is_enabled              BOOLEAN NOT NULL DEFAULT TRUE,
    condition_regex         TEXT NOT NULL,
    parse_regex             TEXT NOT NULL,
    anime_title_source      parser_source_type NOT NULL,
    anime_title_value       VARCHAR(255) NOT NULL,
    episode_no_source       parser_source_type NOT NULL,
    episode_no_value        VARCHAR(50) NOT NULL,
    series_no_source        parser_source_type,
    series_no_value         VARCHAR(50),
    subtitle_group_source   parser_source_type,
    subtitle_group_value    VARCHAR(255),
    resolution_source       parser_source_type,
    resolution_value        VARCHAR(50),
    season_source           parser_source_type,
    season_value            VARCHAR(20),
    year_source             parser_source_type,
    year_value              VARCHAR(10),
    created_at              TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_title_parsers_priority
ON title_parsers(priority DESC)
WHERE is_enabled = TRUE;

-- 建立 raw_anime_items 表
CREATE TABLE raw_anime_items (
    item_id             SERIAL PRIMARY KEY,
    title               TEXT NOT NULL,
    description         TEXT,
    download_url        VARCHAR(2048) NOT NULL,
    pub_date            TIMESTAMP,
    subscription_id     INT NOT NULL REFERENCES subscriptions(subscription_id),
    status              VARCHAR(20) NOT NULL DEFAULT 'pending',
    parser_id           INT REFERENCES title_parsers(parser_id),
    error_message       TEXT,
    parsed_at           TIMESTAMP,
    created_at          TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(download_url)
);

CREATE INDEX idx_raw_items_status ON raw_anime_items(status);
CREATE INDEX idx_raw_items_subscription ON raw_anime_items(subscription_id);
CREATE INDEX idx_raw_items_created ON raw_anime_items(created_at DESC);

-- 修改 anime_links 表，新增 raw_item_id 欄位
ALTER TABLE anime_links
ADD COLUMN raw_item_id INT REFERENCES raw_anime_items(item_id);

CREATE INDEX idx_anime_links_raw_item ON anime_links(raw_item_id);

-- 插入預設解析器
INSERT INTO title_parsers (
    name, description, priority,
    condition_regex, parse_regex,
    anime_title_source, anime_title_value,
    episode_no_source, episode_no_value,
    series_no_source, series_no_value,
    subtitle_group_source, subtitle_group_value,
    resolution_source, resolution_value
) VALUES (
    'LoliHouse 標準格式',
    '匹配 [字幕組] 動畫名稱 - 集數 [解析度] 格式',
    100,
    '^\[.+\].+\s-\s\d+',
    '^\[([^\]]+)\]\s*(.+?)\s+-\s*(\d+)\s*\[.*?(\d{3,4}p)',
    'regex', '2',
    'regex', '3',
    NULL, NULL,
    'regex', '1',
    'regex', '4'
), (
    '六四位元 星號格式',
    '匹配以星號分隔的格式',
    90,
    '^[^★]+★.+★\d+★',
    '^([^★]+)★(.+?)★(\d+)★(\d+x\d+)',
    'regex', '2',
    'regex', '3',
    'static', '1',
    'regex', '1',
    'regex', '4'
), (
    '預設解析器',
    '嘗試匹配任何包含 - 數字 的標題',
    1,
    '.+\s-\s\d+',
    '^(.+?)\s+-\s*(\d+)',
    'regex', '1',
    'regex', '2',
    'static', '1',
    'static', '未知字幕組',
    NULL, NULL
);
