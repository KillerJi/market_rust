use sea_orm::{
    ActiveModelTrait, ActiveValue, ConnectionTrait, DatabaseBackend, Statement, TransactionTrait,
};

use crate::{datas::BoxedResult, entity::*};

use super::StoreDB;
impl StoreDB {
    pub async fn write_block_hight(&self, chain_id: u32, hight: u64) -> BoxedResult<()> {
        let txn = self.pool.begin().await?;
        block::ActiveModel {
            id: ActiveValue::set(chain_id),
            block: ActiveValue::set(hight),
            step: ActiveValue::not_set(),
        }
        .save(&txn)
        .await?;
        txn.commit().await.map_err(|e| e.into())
    }

    pub async fn write_coins_support(
        &self,
        addr: String,
        symbol: String,
        flag: bool,
    ) -> BoxedResult<()> {
        let txn = self.pool.begin().await?;
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            r#"
            INSERT INTO `coins` 
                (`address`, `symbol`, `flag`) 
                VALUES 
                (?, ?, ?)
                ON DUPLICATE KEY UPDATE `flag` = VALUES(`flag`);
        "#,
            vec![addr.into(), symbol.into(), (flag as u8).into()],
        ))
        .await?;
        txn.commit().await.map_err(|e| e.into())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn write_proposals(
        &self,
        proposal_id: u64,
        address: String,
        category: u64,
        token: String,
        state: String,
        liquidity: u128,
        times: [u64; 2],
    ) -> BoxedResult<()> {
        let txn = self.pool.begin().await?;
        let values = vec![
            proposal_id.into(),
            address.into(),
            category.into(),
            token.into(),
            state.into(),
            format!("{}", liquidity).into(),
            times[0].into(),
            times[1].into(),
        ];
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            r#"
            INSERT INTO `proposals`
                (`proposal_id`, `address`, `category`, `token`, `state`, 
                    `liquidity`, `create_time`, `close_time`)
                VALUES
                (?, ?, ?, ?, ?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE
                `address` = VALUES(`address`),
                `category` = VALUES(`category`),
                `token` = VALUES(`token`),
                `state` = VALUES(`state`),
                `liquidity` = VALUES(`liquidity`),
                `create_time` = VALUES(`create_time`),
                `close_time` = VALUES(`close_time`);
            "#,
            values,
        ))
        .await?;
        txn.commit().await.map_err(|e| e.into())
    }

    pub async fn write_relation(
        &self,
        proposal_id: u64,
        account: String,
        relation: String,
    ) -> BoxedResult<()> {
        let txn = self.pool.begin().await?;
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            r#"
            INSERT IGNORE INTO `relations`
                (`proposal_id`, `address`, `relations`)
                VALUES
                (?, ?, ?);
            "#,
            vec![proposal_id.into(), account.into(), relation.into()],
        ))
        .await?;
        txn.commit().await.map_err(|e| e.into())
    }

    pub async fn write_price(
        &self,
        proposal_id: u64,
        ts: u64,
        tokens: [u128; 2],
    ) -> BoxedResult<()> {
        let txn = self.pool.begin().await?;
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            r#"
            INSERT IGNORE INTO `price`
                (`proposal_id`, `ts`, `token1`, `token2`)
                VALUES
                (?, ?, ?, ?);
        "#,
            vec![
                proposal_id.into(),
                ts.into(),
                format!("{}", tokens[0]).into(),
                format!("{}", tokens[1]).into(),
            ],
        ))
        .await?;
        txn.commit().await.map_err(|e| e.into())
    }

    pub async fn write_volume24(&self, proposal_id: u64, volume24: String) -> BoxedResult<()> {
        let txn = self.pool.begin().await?;
        let values = vec![proposal_id.into(), volume24.into()];
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            r#"
            INSERT INTO `proposals`
                (`proposal_id`, `volume24`)
                VALUES
                (?, ?)
                ON DUPLICATE KEY UPDATE
                `volume24` = VALUES(`volume24`);
            "#,
            values,
        ))
        .await?;
        txn.commit().await.map_err(|e| e.into())
    }

    pub async fn write_volume(&self, proposal_id: u64, volume: String) -> BoxedResult<()> {
        let txn = self.pool.begin().await?;
        let values = vec![proposal_id.into(), volume.into()];
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            r#"
            INSERT INTO `proposals`
                (`proposal_id`, `volume`)
                VALUES
                (?, ?)
                ON DUPLICATE KEY UPDATE
                `volume` = VALUES(`volume`);
            "#,
            values,
        ))
        .await?;
        txn.commit().await.map_err(|e| e.into())
    }

    pub async fn write_liquidity(&self, proposal_id: u64, liquidity: String) -> BoxedResult<()> {
        let txn = self.pool.begin().await?;
        let values = vec![proposal_id.into(), liquidity.into()];
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            r#"
            INSERT INTO `proposals`
                (`proposal_id`, `liquidity`)
                VALUES
                (?, ?)
                ON DUPLICATE KEY UPDATE
                `liquidity` = VALUES(`liquidity`);
            "#,
            values,
        ))
        .await?;
        txn.commit().await.map_err(|e| e.into())
    }

    pub async fn write_proposal_state(
        &self,
        proposal_id: String,
        state: String,
    ) -> BoxedResult<()> {
        let txn = self.pool.begin().await?;
        let values = vec![proposal_id.into(), state.into()];
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            r#"
            INSERT INTO `proposals`
                (`proposal_id`, `state`)
                VALUES
                (?, ?)
                ON DUPLICATE KEY UPDATE
                `state` = VALUES(`state`);
            "#,
            values,
        ))
        .await?;
        txn.commit().await.map_err(|e| e.into())
    }

    pub async fn write_proposal_audit_state(
        &self,
        address: String,
        audit_state: String,
    ) -> BoxedResult<()> {
        let txn = self.pool.begin().await?;
        let values = vec![address.into(), audit_state.into()];
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            r#"
            INSERT INTO `proposals`
                (`address`, `audit_state`)
                VALUES
                (?, ?)
                ON DUPLICATE KEY UPDATE
                `audit_state` = VALUES(`audit_state`);
            "#,
            values,
        ))
        .await?;
        txn.commit().await.map_err(|e| e.into())
    }
}
