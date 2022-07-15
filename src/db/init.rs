use super::StoreDB;
use crate::datas::BoxedResult;
use sea_orm::{ConnectionTrait, DatabaseBackend, DbConn, Statement, TransactionTrait};
impl StoreDB {
    pub async fn init_db(pool: DbConn, chain_id: u32) -> BoxedResult<Self> {
        let txn = pool.begin().await?;

        txn.execute(Statement::from_string(
            DatabaseBackend::MySql,
            r#"
            CREATE TABLE IF NOT EXISTS `banner` (
                `id` int unsigned NOT NULL AUTO_INCREMENT,
                `url` varchar(255) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
                PRIMARY KEY (`id`),
                UNIQUE KEY `id` (`id`)
              ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
		"#
            .to_owned(),
        ))
        .await?;

        txn.execute(Statement::from_string(
            DatabaseBackend::MySql,
            r#"
            CREATE TABLE IF NOT EXISTS `block` (
                `id` int(10) unsigned NOT NULL AUTO_INCREMENT,
                `block` BIGINT(20) unsigned NOT NULL,
	            `step` BIGINT(20) unsigned NOT NULL,
                PRIMARY KEY (`id`),
                UNIQUE KEY `id` (`id`)
              ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
		"#
            .to_owned(),
        ))
        .await?;

        txn.execute(Statement::from_string(
            DatabaseBackend::MySql,
            r#"
            CREATE TABLE IF NOT EXISTS `coins` (
                `address` varchar(42) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
                `symbol` varchar(10) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
                `flag` tinyint(1) NOT NULL DEFAULT '0',
                PRIMARY KEY (`address`),
                UNIQUE KEY `address` (`address`)
              ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
		"#
            .to_owned(),
        ))
        .await?;

        txn.execute(Statement::from_string(
            DatabaseBackend::MySql,
            r#"
            CREATE TABLE IF NOT EXISTS `price` (
                `proposal_id` int unsigned NOT NULL AUTO_INCREMENT,
                `ts` int NOT NULL DEFAULT 0,
                `token1` bigint NOT NULL DEFAULT '0',
                `token2` bigint NOT NULL DEFAULT '0',
                PRIMARY KEY (`proposal_id`, `ts`),
                UNIQUE KEY `proposal_id` (`proposal_id`, `ts`)
              ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
		"#
            .to_owned(),
        ))
        .await?;

        txn.execute(Statement::from_string(
            DatabaseBackend::MySql,
            r#"
            CREATE TABLE IF NOT EXISTS `proposals` (
                `proposal_id` int unsigned NOT NULL AUTO_INCREMENT,
                `address` varchar(42) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
                `token` varchar(42) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
                `liquidity` bigint NOT NULL DEFAULT 0,
                `create_time` int NOT NULL DEFAULT 0,
                `close_time` int NOT NULL DEFAULT 0,
                `audit_state` enum('NotReviewed','Passed','NotPassed') CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT 'NotReviewed',
                `category` int NOT NULL DEFAULT 0,
                `state` enum('Original','Formal','End') CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT 'Original',
                `volume` bigint NOT NULL DEFAULT 0,
                `volume24` bigint NOT NULL DEFAULT 0,
                PRIMARY KEY (`proposal_id`),
                UNIQUE KEY `address` (`address`)
              ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
		"#.to_owned(),
        ))
        .await?;

        txn.execute(Statement::from_string(
            DatabaseBackend::MySql,
            r#"
            CREATE TABLE IF NOT EXISTS `relations` (
                `proposal_id` int NOT NULL DEFAULT '0',
                `address` varchar(42) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
                `relations` enum('Liquidity','Create','Trade') CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT '',
                PRIMARY KEY (`proposal_id`,`address`,`relations`) USING BTREE,
                UNIQUE KEY `proposal_id` (`proposal_id`,`address`,`relations`)
              ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
		"#.to_owned(),
        ))
        .await?;

        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            r#"INSERT IGNORE INTO `block` (`id`, `block`, `step`) VALUES
                (?, ?, ?);"#,
            vec![chain_id.into(), 1.into(), 100.into()],
        ))
        .await?;

        txn.commit().await?;

        Ok(Self { pool })
    }
}
