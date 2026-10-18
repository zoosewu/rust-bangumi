//! 作品身分：依 fetcher 回報的 external id 決定季與系列
//! （docs/superpowers/specs/2026-07-15-metadata-identity-design.md）。

use diesel::prelude::*;
use shared::ExternalId;
use std::collections::HashSet;

use crate::models::db::{MetadataNamespace, ModuleTypeEnum};
use crate::schema::{metadata_namespaces, service_modules};

/// metadata 模組註冊時，以這次宣告的 namespace 取代它先前的宣告。
pub fn save_metadata_namespaces(
    conn: &mut PgConnection,
    module_name: &str,
    namespaces: &[String],
) -> QueryResult<()> {
    conn.transaction(|conn| {
        let module_id: i32 = service_modules::table
            .filter(service_modules::name.eq(module_name))
            .select(service_modules::module_id)
            .first(conn)?;
        diesel::delete(
            metadata_namespaces::table.filter(metadata_namespaces::module_id.eq(module_id)),
        )
        .execute(conn)?;
        let rows: Vec<MetadataNamespace> = namespaces
            .iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .map(|namespace| MetadataNamespace {
                module_id,
                namespace: namespace.clone(),
                priority: 0,
            })
            .collect();
        diesel::insert_into(metadata_namespaces::table)
            .values(&rows)
            .execute(conn)?;
        Ok(())
    })
}

/// 目前啟用中的 metadata service 能解析的 namespace。
pub fn registered_namespaces(conn: &mut PgConnection) -> QueryResult<HashSet<String>> {
    metadata_namespaces::table
        .inner_join(service_modules::table)
        .filter(service_modules::is_enabled.eq(true))
        .filter(service_modules::module_type.eq(ModuleTypeEnum::Metadata))
        .select(metadata_namespaces::namespace)
        .distinct()
        .load::<String>(conn)
        .map(|namespaces| namespaces.into_iter().collect())
}

/// fetcher 回報的身分中，有 metadata service 能解析的那些。
pub fn usable_ids(ids: &[ExternalId], registered: &HashSet<String>) -> Vec<ExternalId> {
    ids.iter()
        .filter(|id| registered.contains(&id.namespace))
        .cloned()
        .collect()
}

#[cfg(test)]
pub(crate) mod test_db {
    use diesel::prelude::*;

    /// 開發資料庫連線（`DATABASE_TEST_URL`），整個測試在 rollback 的 transaction 裡跑。
    pub fn conn() -> PgConnection {
        let url = std::env::var("DATABASE_TEST_URL")
            .expect("DATABASE_TEST_URL must point to a migrated dev database");
        let mut conn = PgConnection::establish(&url).expect("connect to DATABASE_TEST_URL");
        conn.begin_test_transaction()
            .expect("begin test transaction");
        conn
    }

    pub fn insert_module(conn: &mut PgConnection, name: &str, enabled: bool) -> i32 {
        diesel::sql_query(
            "INSERT INTO service_modules (module_type, name, version, is_enabled, priority, base_url, created_at, updated_at) \
             VALUES ('metadata', $1, '1', $2, 50, 'http://m', now(), now())",
        )
        .bind::<diesel::sql_types::Text, _>(name)
        .bind::<diesel::sql_types::Bool, _>(enabled)
        .execute(conn)
        .unwrap();
        crate::schema::service_modules::table
            .filter(crate::schema::service_modules::name.eq(name))
            .select(crate::schema::service_modules::module_id)
            .first(conn)
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn a_metadata_service_registering_twice_declares_its_namespaces_once() {
        let mut conn = test_db::conn();
        test_db::insert_module(&mut conn, "meta-a", true);
        save_metadata_namespaces(&mut conn, "meta-a", &["bgm".to_string()]).unwrap();
        save_metadata_namespaces(&mut conn, "meta-a", &["bgm".to_string()]).unwrap();
        assert_eq!(registered_namespaces(&mut conn).unwrap(), set(&["bgm"]));
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn a_new_declaration_replaces_the_old_one() {
        let mut conn = test_db::conn();
        test_db::insert_module(&mut conn, "meta-a", true);
        save_metadata_namespaces(&mut conn, "meta-a", &["bgm".to_string()]).unwrap();
        save_metadata_namespaces(&mut conn, "meta-a", &["tmdb".to_string()]).unwrap();
        assert_eq!(registered_namespaces(&mut conn).unwrap(), set(&["tmdb"]));
    }

    #[test]
    #[ignore = "needs DATABASE_TEST_URL"]
    fn a_disabled_service_resolves_nothing() {
        let mut conn = test_db::conn();
        test_db::insert_module(&mut conn, "meta-off", false);
        save_metadata_namespaces(&mut conn, "meta-off", &["bgm".to_string()]).unwrap();
        assert!(registered_namespaces(&mut conn).unwrap().is_empty());
    }

    #[test]
    fn only_registered_namespaces_are_usable() {
        let ids = vec![
            ExternalId::new("mikan", "3822"),
            ExternalId::new("bgm", "548818"),
        ];
        assert_eq!(
            usable_ids(&ids, &set(&["bgm"])),
            vec![ExternalId::new("bgm", "548818")]
        );
        assert!(usable_ids(&ids, &set(&[])).is_empty());
    }
}
