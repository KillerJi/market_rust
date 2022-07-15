use std::str::FromStr;

use sea_orm::{
    sea_query::{BinOper, Expr, IntoColumnRef, SimpleExpr},
    ColumnTrait, Condition, DeriveColumn, EntityTrait, EnumIter, IdenStatic, Order, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, 
};

use super::StoreDB;
use crate::{
    datas::{handle::CombineMap, BoxedResult},
    db::FromSymbol,
    entity::{prelude::*, *},
};
impl StoreDB {
    pub async fn read_coins_support(&self) -> BoxedResult<Vec<(String, String, bool)>> {
        #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
        enum QueryAs {
            Address,
            Symbol,
            Flag,
        }
        Coins::find()
            .select_only()
            .column_as(coins::Column::Address, QueryAs::Address)
            .column_as(coins::Column::Symbol, QueryAs::Symbol)
            .column_as(coins::Column::Flag, QueryAs::Flag)
            .into_values::<_, QueryAs>()
            .all(&self.pool)
            .await
            .map_err(|e| e.into())
    }

    pub async fn read_block(&self, chain_id: u32) -> BoxedResult<(u64, u64)> {
        #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
        enum QueryAs {
            Block,
            Step,
        }
        Block::find_by_id(chain_id)
            .select_only()
            .column_as(block::Column::Block, QueryAs::Block)
            .column_as(block::Column::Step, QueryAs::Step)
            .into_values::<_, QueryAs>()
            .one(&self.pool)
            .await?
            .ok_or_else(|| "id not found".into())
    }

    pub async fn read_proposals(&self) -> BoxedResult<Vec<(u64, String, String)>> {
        #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
        enum QueryAs {
            ProposalId,
            Address,
            State,
        }
        Proposals::find()
            .select_only()
            .column_as(proposals::Column::ProposalId, QueryAs::ProposalId)
            .column_as(proposals::Column::Address, QueryAs::Address)
            .column_as(proposals::Column::State, QueryAs::State)
            .into_values::<_, QueryAs>()
            .all(&self.pool)
            .await
            .map_err(|e| e.into())
    }

    pub async fn read_relation(&self, account: String, relation: String) -> BoxedResult<Vec<u64>> {
        #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
        enum QueryAs {
            ProposalId,
        }
        Relations::find()
            .select_only()
            .column_as(relations::Column::ProposalId, QueryAs::ProposalId)
            .filter(
                Condition::all()
                    .add(relations::Column::Address.eq(account))
                    .add(relations::Column::Relations.eq(relation)),
            )
            .into_values::<_, QueryAs>()
            .all(&self.pool)
            .await
            .map_err(|e| e.into())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn read_list(
        &self,
        count: usize,
        page: usize,
        combine: &CombineMap,
        order_map: &[(&str, bool)],
        dup: Option<Vec<u64>>,
    ) -> BoxedResult<(usize, Vec<(u64, u64, String, String)>)> {
        #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
        enum QueryAs {
            ProposalId,
            CloseTime,
            Address,
            State,
        }

        let condition = combine
            .iter()
            .fold(Condition::all(), |condition, (op, map)| {
                //处理op运算符的每一个map
                if let Ok(op) = BinOper::from_str(op) {
                    map.iter().filter(|(_, value)| !value.is_empty()).fold(
                        condition,
                        |condition, (key, value)| {
                            if let Ok(key) = proposals::Column::from_str(key) {
                                condition.add(SimpleExpr::Binary(
                                    Box::new(SimpleExpr::Column(key.into_column_ref())),
                                    op,
                                    Box::new(SimpleExpr::Value(value.to_owned().into())),
                                ))
                            } else {
                                condition
                            }
                        },
                    )
                } else {
                    condition
                }
            });

        let condition = if let Some(dup) = dup {
            condition.add(Expr::col(proposals::Column::ProposalId).is_in(dup))
        } else {
            condition
        };

        let prepare = Proposals::find()
            .select_only()
            .column_as(proposals::Column::ProposalId, QueryAs::ProposalId)
            .column_as(proposals::Column::CloseTime, QueryAs::CloseTime)
            .column_as(proposals::Column::Address, QueryAs::Address)
            .column_as(proposals::Column::State, QueryAs::State)
            .filter(condition);

        let prepare = order_map.iter().fold(prepare, |prepare, (key, value)| {
            if let Ok(key) = proposals::Column::from_str(key) {
                let ord = [Order::Desc, Order::Asc][*value as usize];
                prepare.order_by(key, ord)
            } else {
                prepare
            }
        });
        let paginator = prepare
            .into_values::<_, QueryAs>()
            .paginate(&self.pool, count);
        let total = paginator.num_pages().await?;

        let r = paginator.fetch_page(page - 1).await?;
        Ok((total, r))
    }

    pub async fn query_banner(&self) -> BoxedResult<Vec<String>> {
        #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
        enum QueryAs {
            Url,
        }
        Banner::find()
            .select_only()
            .column_as(banner::Column::Url, QueryAs::Url)
            .into_values::<_, QueryAs>()
            .all(&self.pool)
            .await
            .map_err(|e| e.into())
    }

    pub async fn read_proposal_id(
        &self,
        token: String,
        count: usize,
        page: usize,
    ) -> BoxedResult<(usize, Vec<u64>)> {
        #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
        enum QueryAs {
            ProposalId,
        }
        let paginator = Proposals::find()
            .select_only()
            .column_as(proposals::Column::ProposalId, QueryAs::ProposalId)
            .filter(Condition::all().add(proposals::Column::Token.eq(token)))
            .into_values::<_, QueryAs>()
            .paginate(&self.pool, count);
        let total = paginator.num_pages().await?;

        let r = paginator.fetch_page(page - 1).await?;
        Ok((total, r))
    }

    pub async fn read_history_proposal_id(
        &self,
        combine: &CombineMap,
    ) -> BoxedResult<Vec<(u64, u64, u64)>> {
        #[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
        enum QueryAs {
            Ts,
            Token1,
            Token2,
        }

        let condition = combine
            .iter()
            .fold(Condition::all(), |condition, (op, map)| {
                //处理op运算符的每一个map
                if let Ok(op) = BinOper::from_str(op) {
                    
                    map.iter().filter(|(_, value)| !value.is_empty()).fold(
                        condition,
                        |condition, (key, value)| {
                            if let Ok(key) = price::Column::from_str(key) {
                                condition.add(SimpleExpr::Binary(
                                    Box::new(SimpleExpr::Column(key.into_column_ref())),
                                    op,
                                    Box::new(SimpleExpr::Value(value.to_owned().into())),
                                ))
                            } else {
                                condition
                            }
                        },
                    )
                } else {
                    condition
                }
            });
            
        Price::find()
            .filter(condition)
            .select_only()
            .column_as(price::Column::Ts, QueryAs::Ts)
            .column_as(price::Column::Token1, QueryAs::Token1)
            .column_as(price::Column::Token2, QueryAs::Token2)
            .into_values::<_, QueryAs>()
            .all(&self.pool)
            .await
            .map_err(|e| e.into())
    }
}
