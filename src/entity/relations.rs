//! SeaORM Entity. Generated by sea-orm-codegen 0.6.0

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "relations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub proposal_id: u64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub address: String,
    #[sea_orm(
        primary_key,
        auto_increment = false,
        column_type = "Custom(\"ENUM ('Liquidity','Create','Trade')\".to_owned())"
    )]
    pub relations: String,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No RelationDef")
    }
}

impl ActiveModelBehavior for ActiveModel {}
