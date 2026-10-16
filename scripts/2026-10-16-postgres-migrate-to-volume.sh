#!/usr/bin/env bash
# 2026-10-16 資料庫修復：PostgreSQL 從外接硬碟 bind mount 搬到 Docker named volume
#
# 背景：資料目錄 bind mount 在 macOS USB 外接硬碟（經 OrbStack 檔案共享）上，
# 生產出現寫入遺失：索引缺列（downloads 505、cron_logs 2330 列）、主鍵重複
# （cron_logs 1165 組、viewer sync_tasks 2 組）、postmaster 未重啟序列卻倒退。
# 詳見 docs/runbooks/2026-10-16-production-environment-audit.md E1。
#
# 作法：pg_dump（COPY 走 seq scan，不依賴損壞的索引）→ 新 cluster（named volume、
# --data-checksums）→ pg_restore 分段：pre-data → data → 修正重複主鍵 →
# post-data（此時才建立 PK / 索引 / FK）→ 序列對齊 max(id) → 驗證。
#
# 舊資料目錄（POSTGRES_DATA_DIR，預設 ./data/postgres）全程不動，可隨時回滾。
#
# 使用方式（生產主機、專案根目錄，且已 git pull 到含 postgres_data volume 的版本）：
#   scripts/2026-10-16-postgres-migrate-to-volume.sh preflight
#   scripts/2026-10-16-postgres-migrate-to-volume.sh dump      # 停應用服務、匯出
#   scripts/2026-10-16-postgres-migrate-to-volume.sh switch    # 以新 volume 啟動 postgres
#   scripts/2026-10-16-postgres-migrate-to-volume.sh restore   # 匯入 + 修正
#   scripts/2026-10-16-postgres-migrate-to-volume.sh verify    # 全部 PASS 才往下
#   scripts/2026-10-16-postgres-migrate-to-volume.sh start     # 啟動應用服務
# 回滾（任何階段失敗時）：
#   scripts/2026-10-16-postgres-migrate-to-volume.sh rollback
set -euo pipefail

cd "$(dirname "$0")/.."
export PATH="/usr/local/bin:$PATH"

PG=bangumi-postgres
PROJECT=rust-bangumi
NEW_VOLUME="${PROJECT}_postgres_data"
OLD_COMMIT=816a09f
BACKUP_DIR=data/backups/2026-10-16-pg
APP_SERVICES=(core-service fetcher-mikanani downloader-qbittorrent pikpak-downloader viewer-jellyfin metadata frontend)
VIEWER_DB=viewer_jellyfin

log() { printf '\n==> %s\n' "$*"; }
die() { printf '\n!! %s\n' "$*" >&2; exit 1; }
# 過濾 compose 的 version 過時警告，但保留 docker compose 本身的結束碼
compose() {
  local out rc=0
  out=$(docker compose "$@" 2>&1) || rc=$?
  printf '%s\n' "$out" | grep -v 'attribute `version` is obsolete' || true
  return "$rc"
}

pg_env() { docker exec "$PG" printenv "$1"; }
psql_db() { docker exec -i "$PG" psql -U "$PGU" -d "$1" -v ON_ERROR_STOP=1 -X -q "${@:2}"; }

wait_healthy() {
  for _ in $(seq 1 60); do
    docker exec "$PG" pg_isready -U "$PGU" -q 2>/dev/null && return 0
    sleep 2
  done
  die "postgres 未在 120 秒內就緒"
}

# 以 seq scan 計算每張表筆數（不經索引），輸出 "table|count"
count_rows_sql() {
  cat <<'SQL'
\pset tuples_only on
\pset format unaligned
set enable_indexscan=off; set enable_bitmapscan=off; set enable_indexonlyscan=off;
create temp table _c(tbl text, n bigint);
do $$ declare t record; n bigint; begin
  for t in select tablename from pg_tables where schemaname='public' order by 1 loop
    execute format('select count(*) from %I', t.tablename) into n;
    insert into _c values (t.tablename, n);
  end loop; end $$;
select tbl || '|' || n from _c order by tbl;
SQL
}

# 驗證：seq scan 與 PK 索引筆數一致、唯一索引無重複、序列 >= max(id)
verify_sql() {
  cat <<'SQL'
\pset tuples_only on
\pset format unaligned
create temp table _v(problem text);
do $$ declare t record; s bigint; i bigint; n bigint; mx bigint; lv bigint; begin
  -- PK：seq scan vs index scan
  for t in select c.relname tbl, a.attname col from pg_index x
      join pg_class c on c.oid=x.indrelid
      join pg_attribute a on a.attrelid=c.oid and a.attnum=x.indkey[0]
      join pg_namespace ns on ns.oid=c.relnamespace
      where x.indisprimary and ns.nspname='public' loop
    set local enable_indexscan=off; set local enable_bitmapscan=off; set local enable_indexonlyscan=off;
    execute format('select count(*) from %I', t.tbl) into s;
    set local enable_seqscan=off; set local enable_indexscan=on; set local enable_indexonlyscan=on; set local enable_bitmapscan=on;
    execute format('select count(*) from %I where %I is not null', t.tbl, t.col) into i;
    set local enable_seqscan=on;
    if s <> i then insert into _v values (format('index mismatch %s seq=%s idx=%s', t.tbl, s, i)); end if;
  end loop;
  -- 唯一索引：重複值
  for t in select c.relname tbl, ic.relname idx,
      (select string_agg(quote_ident(a.attname), ',') from unnest(x.indkey) k
         join pg_attribute a on a.attrelid=c.oid and a.attnum=k) cols,
      pg_get_expr(x.indpred, x.indrelid) pred
      from pg_index x join pg_class c on c.oid=x.indrelid join pg_class ic on ic.oid=x.indexrelid
      join pg_namespace ns on ns.oid=c.relnamespace
      where x.indisunique and ns.nspname='public' loop
    execute format('select count(*) from (select 1 from %I %s group by %s having count(*)>1) z',
                   t.tbl, coalesce('where '||t.pred, ''), t.cols) into n;
    if n > 0 then insert into _v values (format('duplicates %s.%s groups=%s', t.tbl, t.idx, n)); end if;
  end loop;
  -- 序列落後 max(id)（max 也強制 seq scan，避免被索引誤導）
  set local enable_indexscan=off; set local enable_bitmapscan=off; set local enable_indexonlyscan=off;
  for t in select s2.relname seq, c.relname tbl, a.attname col from pg_class s2
      join pg_depend d on d.objid=s2.oid and d.deptype in ('a','i')
      join pg_class c on c.oid=d.refobjid
      join pg_attribute a on a.attrelid=c.oid and a.attnum=d.refobjsubid
      where s2.relkind='S' loop
    execute format('select max(%I) from %I', t.col, t.tbl) into mx;
    execute format('select last_value from %I', t.seq) into lv;
    if mx is not null and lv < mx then insert into _v values (format('sequence %s last=%s < max=%s', t.seq, lv, mx)); end if;
  end loop;
end $$;
select coalesce(string_agg(problem, E'\n'), 'OK') from _v;
SQL
}

# 序列對齊 max(id)
setval_sql() {
  cat <<'SQL'
do $$ declare t record; mx bigint; begin
  set local enable_indexscan=off; set local enable_bitmapscan=off; set local enable_indexonlyscan=off;
  for t in select s.relname seq, c.relname tbl, a.attname col from pg_class s
      join pg_depend d on d.objid=s.oid and d.deptype in ('a','i')
      join pg_class c on c.oid=d.refobjid
      join pg_attribute a on a.attrelid=c.oid and a.attnum=d.refobjsubid
      where s.relkind='S' loop
    execute format('select max(%I) from %I', t.col, t.tbl) into mx;
    if mx is not null then
      execute format('select setval(%L, greatest(%s, (select last_value from %I)))', t.seq, mx, t.seq);
    end if;
  end loop; end $$;
SQL
}

# 重複主鍵：保留每組最早的一列，其餘改編到 max+1 之後（尚未建立 PK，以 ctid 定位）
dedup_sql_bangumi() {
  cat <<'SQL'
with dup as (select ctid, row_number() over (partition by log_id order by executed_at, ctid) rn from cron_logs),
     ren as (select ctid, row_number() over (order by ctid) k from dup where rn > 1),
     base as (select max(log_id) mx from cron_logs)
update cron_logs c set log_id = base.mx + ren.k from ren, base where c.ctid = ren.ctid;
SQL
}
dedup_sql_viewer() {
  cat <<'SQL'
with dup as (select ctid, row_number() over (partition by task_id order by completed_at nulls last, ctid) rn from sync_tasks),
     ren as (select ctid, row_number() over (order by ctid) k from dup where rn > 1),
     base as (select max(task_id) mx from sync_tasks)
update sync_tasks c set task_id = base.mx + ren.k from ren, base where c.ctid = ren.ctid;
SQL
}

mount_source() {
  docker inspect -f '{{range .Mounts}}{{if eq .Destination "/var/lib/postgresql/data"}}{{.Type}}:{{if .Name}}{{.Name}}{{else}}{{.Source}}{{end}}{{end}}{{end}}' "$PG"
}

PGU=$(pg_env POSTGRES_USER 2>/dev/null || echo bangumi)
MAIN_DB=$(pg_env POSTGRES_DB 2>/dev/null || echo bangumi)
DBS=("$MAIN_DB" "$VIEWER_DB")

phase_preflight() {
  log "目前 postgres 資料目錄：$(mount_source)"
  grep -q 'postgres_data:/var/lib/postgresql/data' docker-compose.yaml \
    || die "docker-compose.yaml 尚未包含 postgres_data volume，請先 git pull"
  if docker volume inspect "$NEW_VOLUME" >/dev/null 2>&1; then
    die "volume $NEW_VOLUME 已存在；若是上次失敗的殘留，確認後執行 docker volume rm $NEW_VOLUME"
  fi
  [ -e "$BACKUP_DIR/${MAIN_DB}.dump" ] && die "$BACKUP_DIR 已有備份，避免覆蓋請先改名"
  log "OK：可以執行 dump"
}

phase_dump() {
  log "停止應用服務（qbittorrent / jellyfin 保持運作）"
  # 先停 core：若 downloader 先停，core 的輪詢會把所有 downloading 標成 downloader_error
  compose stop core-service
  compose stop "${APP_SERVICES[@]}"
  local clients
  clients=$(docker exec "$PG" psql -U "$PGU" -d postgres -Atc \
    "select count(*) from pg_stat_activity where datname is not null and pid<>pg_backend_pid()")
  [ "$clients" = "0" ] || die "仍有 $clients 個 DB 連線，請確認後重試"

  mkdir -p "$BACKUP_DIR"
  docker exec "$PG" pg_dumpall -U "$PGU" --globals-only > "$BACKUP_DIR/globals.sql"
  for db in "${DBS[@]}"; do
    log "匯出 $db"
    docker exec "$PG" pg_dump -U "$PGU" -d "$db" -Fc -f "/tmp/$db.dump"
    docker cp "$PG:/tmp/$db.dump" "$BACKUP_DIR/$db.dump"
    docker exec "$PG" rm -f "/tmp/$db.dump"
    count_rows_sql | psql_db "$db" > "$BACKUP_DIR/$db.counts"
    echo "  $(wc -l < "$BACKUP_DIR/$db.counts") 張表，$(du -h "$BACKUP_DIR/$db.dump" | cut -f1)"
  done
  log "OK：備份在 ${BACKUP_DIR}，下一步 switch"
}

phase_switch() {
  [ -s "$BACKUP_DIR/${MAIN_DB}.dump" ] && [ -s "$BACKUP_DIR/${VIEWER_DB}.dump" ] || die "找不到備份，請先 dump"
  log "以 named volume 重建 postgres（舊資料目錄保留不動）"
  compose stop postgres
  compose rm -f postgres
  compose up -d postgres
  wait_healthy
  local src checksums
  src=$(mount_source)
  checksums=$(docker exec "$PG" psql -U "$PGU" -d postgres -Atc "show data_checksums")
  echo "  資料目錄：${src}，data_checksums=$checksums"
  [ "$src" = "volume:$NEW_VOLUME" ] || die "postgres 沒有掛在 $NEW_VOLUME"
  [ "$checksums" = "on" ] || die "data_checksums 未開啟"
  docker exec "$PG" psql -U "$PGU" -d postgres -Atc "select 1 from pg_database where datname='$VIEWER_DB'" | grep -q 1 \
    || die "$VIEWER_DB 未由 init script 建立"
  log "OK：下一步 restore"
}

restore_db() {
  local db=$1 dedup=$2
  log "匯入 $db"
  docker cp "$BACKUP_DIR/$db.dump" "$PG:/tmp/$db.dump"
  docker exec "$PG" pg_restore -U "$PGU" -d "$db" --exit-on-error --section=pre-data "/tmp/$db.dump"
  docker exec "$PG" pg_restore -U "$PGU" -d "$db" --exit-on-error --section=data "/tmp/$db.dump"
  echo "  修正重複主鍵"
  "$dedup" | psql_db "$db"
  docker exec "$PG" pg_restore -U "$PGU" -d "$db" --exit-on-error --section=post-data "/tmp/$db.dump"
  setval_sql | psql_db "$db"
  docker exec "$PG" rm -f "/tmp/$db.dump"
  docker exec "$PG" psql -U "$PGU" -d "$db" -qc "analyze"
}

phase_restore() {
  [ "$(mount_source)" = "volume:$NEW_VOLUME" ] || die "postgres 尚未切換到 ${NEW_VOLUME}，請先 switch"
  restore_db "$MAIN_DB" dedup_sql_bangumi
  restore_db "$VIEWER_DB" dedup_sql_viewer
  log "OK：下一步 verify"
}

phase_verify() {
  local failed=0
  for db in "${DBS[@]}"; do
    log "驗證 $db"
    if diff <(count_rows_sql | psql_db "$db") "$BACKUP_DIR/$db.counts" >/dev/null; then
      echo "  筆數：與匯出時一致"
    else
      echo "  筆數：不一致"; diff <(count_rows_sql | psql_db "$db") "$BACKUP_DIR/$db.counts" || true; failed=1
    fi
    local result
    result=$(verify_sql | psql_db "$db" | grep -v '^$' | tail -1)
    echo "  索引／重複／序列：$result"
    [ "$result" = "OK" ] || { verify_sql | psql_db "$db"; failed=1; }
  done
  [ "$failed" = 0 ] || die "驗證失敗：不要啟動應用服務，視情況 rollback"
  log "PASS：下一步 start"
}

phase_start() {
  log "啟動應用服務"
  compose start "${APP_SERVICES[@]}"
  compose ps
  log "請觀察：docker logs -f bangumi-core（無 DB 錯誤）、viewer 啟動遷移正常"
}

phase_rollback() {
  log "回滾：以舊 compose（${OLD_COMMIT}）與舊資料目錄重啟 postgres"
  local old=/tmp/rust-bangumi-compose-$OLD_COMMIT.yaml
  git show "$OLD_COMMIT:docker-compose.yaml" > "$old"
  compose stop postgres
  compose rm -f postgres
  compose -p "$PROJECT" --project-directory . -f "$old" -f docker-compose.override.yaml up -d postgres
  wait_healthy
  echo "  資料目錄：$(mount_source)"
  compose start "${APP_SERVICES[@]}"
  log "已回滾；新 volume $NEW_VOLUME 保留供調查，確認不需要後再 docker volume rm"
}

case "${1:-}" in
  preflight) phase_preflight ;;
  dump) phase_dump ;;
  switch) phase_switch ;;
  restore) phase_restore ;;
  verify) phase_verify ;;
  start) phase_start ;;
  rollback) phase_rollback ;;
  *) sed -n '2,25p' "$0"; exit 1 ;;
esac
