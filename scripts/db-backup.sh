#!/bin/sh
# 定期備份資料庫（docker compose 的 db-backup 服務執行）。
#
# 每 BACKUP_INTERVAL_SECONDS 秒對 BACKUP_DATABASES 裡的每個資料庫做一次 pg_dump（custom 格式），
# 存到 BACKUP_DIR。備份先寫成暫存檔，用 pg_restore --list 確認能讀取後才改成正式檔名；
# 超過 BACKUP_KEEP_DAYS 天的備份會刪除，但每個資料庫至少保留最新 BACKUP_KEEP_MIN 份，
# 避免備份連續失敗一段時間後把僅存的備份也清掉。
#
# 連線用標準的 PGHOST / PGUSER / PGPASSWORD 環境變數。
# BACKUP_ONCE=1：只備份一次就結束（測試或手動立即備份）。
#
# 還原（會先清掉目標資料庫裡同名的物件）：
#   docker exec -i bangumi-postgres pg_restore -U "$POSTGRES_USER" -d bangumi --clean --if-exists < data/backups/bangumi-YYYYmmdd-HHMMSS.dump
set -eu

BACKUP_DIR="${BACKUP_DIR:-/backups}"
BACKUP_DATABASES="${BACKUP_DATABASES:?BACKUP_DATABASES is required}"
BACKUP_INTERVAL_SECONDS="${BACKUP_INTERVAL_SECONDS:-86400}"
BACKUP_KEEP_DAYS="${BACKUP_KEEP_DAYS:-14}"
BACKUP_KEEP_MIN="${BACKUP_KEEP_MIN:-3}"

log() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') $*"
}

# 刪掉超過保留天數的備份，但保留最新的 BACKUP_KEEP_MIN 份
prune() {
    db="$1"
    ls -1t "$BACKUP_DIR/$db"-*.dump 2>/dev/null | tail -n +"$((BACKUP_KEEP_MIN + 1))" | while read -r old; do
        if [ -n "$(find "$old" -mtime +"$BACKUP_KEEP_DAYS")" ]; then
            rm -f "$old"
            log "removed old backup $(basename "$old")"
        fi
    done
}

backup_once() {
    stamp="$(date '+%Y%m%d-%H%M%S')"
    failed=0
    for db in $BACKUP_DATABASES; do
        partial="$BACKUP_DIR/.$db-$stamp.dump.partial"
        final="$BACKUP_DIR/$db-$stamp.dump"
        if pg_dump --format=custom --dbname="$db" --file="$partial" && pg_restore --list "$partial" >/dev/null; then
            mv "$partial" "$final"
            log "backed up $db -> $(basename "$final") ($(du -h "$final" | cut -f1))"
            prune "$db"
        else
            rm -f "$partial"
            log "ERROR: backup of $db failed"
            failed=1
        fi
    done
    return "$failed"
}

mkdir -p "$BACKUP_DIR"
until pg_isready --quiet; do
    log "waiting for database"
    sleep 5
done

if [ "${BACKUP_ONCE:-0}" = "1" ]; then
    backup_once
    exit $?
fi

log "backing up [$BACKUP_DATABASES] every ${BACKUP_INTERVAL_SECONDS}s into $BACKUP_DIR (keep ${BACKUP_KEEP_DAYS} days, at least ${BACKUP_KEEP_MIN})"
while true; do
    backup_once || true
    sleep "$BACKUP_INTERVAL_SECONDS"
done
