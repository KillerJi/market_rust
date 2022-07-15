mod init;
mod read;
pub(crate) mod write;

use sea_orm::{DbConn, sea_query::BinOper};

#[derive(Clone)]
pub struct StoreDB {
    pool: DbConn,
}

pub trait FromSymbol: Sized {
    type Err;
    fn from_str(s: &str) -> Result<Self, Self::Err>;
}

impl FromSymbol for BinOper {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, &'static str> {
        match s {
            ">" => Ok(Self::GreaterThan),
            "<" => Ok(Self::SmallerThan),
            ">=" => Ok(Self::GreaterThanOrEqual),
            "<=" => Ok(Self::SmallerThanOrEqual),
            "=" => Ok(Self::Equal),
            "!=" => Ok(Self::NotEqual),
            _ => Err("connot convert"),
        }
    }
}