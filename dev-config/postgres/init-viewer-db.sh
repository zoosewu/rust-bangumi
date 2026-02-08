#!/bin/bash
# 自動建立 viewer_jellyfin 資料庫（docker-entrypoint-initdb.d 會在首次啟動時執行）
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    SELECT 'CREATE DATABASE viewer_jellyfin OWNER $POSTGRES_USER'
    WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'viewer_jellyfin')\gexec
EOSQL

echo "viewer_jellyfin database ready"
