-- 新增 catch-all 解析器：匹配所有標題，整個文字作為動畫名稱，集數預設 1
INSERT INTO title_parsers (
    name, description, priority,
    condition_regex, parse_regex,
    anime_title_source, anime_title_value,
    episode_no_source, episode_no_value,
    series_no_source, series_no_value,
    subtitle_group_source, subtitle_group_value
) VALUES (
    'Catch-All 全匹配',
    '最低優先級，將整個標題作為動畫名稱，集數預設為 1。確保所有標題都能被解析。',
    0,
    '.+',
    '^(.+)$',
    'regex', '1',
    'static', '1',
    'static', '1',
    'static', '未知字幕組'
);
