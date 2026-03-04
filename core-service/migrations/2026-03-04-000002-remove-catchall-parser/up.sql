-- 移除 Catch-All 解析器（priority=0，條件 .+，名稱含「全匹配」）
DELETE FROM title_parsers WHERE name = 'Catch-All 全匹配';
