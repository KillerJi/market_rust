use std::{collections::HashMap, str::FromStr, time::SystemTime};

use actix_web::{http::StatusCode, web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};

use crate::{actors::ws::XProtocolWs, db::StoreDB};

use super::{data::AppData, error::XProtocolError};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XResponse<T> {
    pub code: u16,
    pub data: T,
}

impl<T> XResponse<T> {
    pub fn new(code: StatusCode, data: T) -> Self {
        Self {
            code: code.as_u16(),
            data,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Coins {
    pub address: String,
    pub symbol: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct History {
    pub ts: u64,
    pub t1: Vec<String>,
    pub t2: Vec<String>
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ProposalState {
    Original,
    Formal,
    End,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Hash, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ProposalAduitState {
    NotReviewed,
    Passed,
    NotPassed,
}

#[derive(Debug)]
pub enum ProposalRelation {
    Liquidity,
    Create,
    Trade,
}

impl TryFrom<u32> for ProposalRelation {
    type Error = XProtocolError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Liquidity),
            1 => Ok(Self::Create),
            2 => Ok(Self::Trade),
            _ => Err(XProtocolError::ExpectationFailed),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Pagination {
    pub page: usize,
    pub count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProposalItem {
    pub proposal_id: u64,
    pub create_time: u64,
    pub address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProposalList<T> {
    pub total: usize,
    pub current: usize,
    pub list: T,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub code: u16,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(code: StatusCode, message: String) -> Self {
        Self {
            code: code.as_u16(),
            message,
        }
    }
}

pub type CombineMap = HashMap<&'static str, HashMap<String, String>>;
pub struct Handlers;

impl Handlers {
    pub fn app_config(cfg: &mut web::ServiceConfig) {
        cfg.route("/ws", web::get().to(Self::index))
            .route("/coins", web::get().to(Self::coins_support))
            .route("/categories/{filter}", web::get().to(Self::categories_support))
            .route("/original/{audit_state}", web::get().to(Self::original))
            .route("/formal/{status}", web::get().to(Self::select_proposal))
            .route("/banners", web::get().to(Self::banner))
            .route("/backstage/{token}", web::get().to(Self::backstage))
            .route("/history/{id}", web::get().to(Self::history));
        //ws
    }

    pub async fn history(
        data: web::Data<AppData>,
        path: web::Path<String>,
    ) -> Result<HttpResponse, XProtocolError> {
        let now = Self::now();
        let start_date = now - 2592000;
        let mut combine = CombineMap::new();
        let proposal_id = path.into_inner();
        Self::insert_combine(&mut combine, "<=", "ts".into(), format!("{:?}", now));
        Self::insert_combine(&mut combine, ">=", "ts".into(), format!("{:?}", start_date));
        Self::insert_combine(
            &mut combine,
            "=",
            "proposal_id".into(),
            proposal_id,
        );
        if let Ok(history_price) = data.store.read_history_proposal_id(&combine).await {
            if history_price.is_empty() {
                Ok(HttpResponse::Ok().json(XResponse::new(StatusCode::OK, vec![0])))
            } else {
                let price = history_price
                    .iter()
                    .map(|(ts, a, _)| {
                        (((ts - start_date) / 86400 + 1), *a as u128, 0)
                    })
                    .fold(
                        HashMap::<u64, (u128, u128)>::new(),
                        |mut acc, (ts, a, _)| {
                            acc.entry(ts)
                                .and_modify(|v| {
                                    v.0 += a;
                                    v.1 += 1;
                                })
                                .or_insert((a, 1));
                            acc
                        },
                    )
                    .iter()
                    .map(|(&k, &v)| (k, (v.0 / v.1)))
                    .collect::<HashMap<u64, u128>>();
                    let mut prev0 = "0.50".to_string();
                    let mut prev1 = "0.50".to_string();
                let x :Vec<u64>= (1..31).collect();
                let x = x.iter().map(|&i|{
                    if let Some(&v) = price.get(&i){
                        prev0 = format!("0.{}", v);
                        prev1 = format!("0.{}", 100 - v);
                    }
                    (i, (prev0.clone(), prev1.clone()))
                }).fold(Vec::<(u64,String,String)>::new(),|mut acc,(t,(p1,p2))|{
                    acc.push((t,p1,p2));
                    acc
                });
                Ok(HttpResponse::Ok().json(XResponse::new(StatusCode::OK, x)))
            }
        } else {
            Err(XProtocolError::InternalServerError)
        }
    }

    pub async fn backstage(
        req: HttpRequest,
        data: web::Data<AppData>,
        path: web::Path<String>,
        info: web::Query<Pagination>,
    ) -> Result<HttpResponse, XProtocolError> {
        let token = path.into_inner();
        let (_, page, count) = Self::get_common(req, info)?;
        let dup = data
            .store
            .read_proposal_id(token, count, page)
            .await
            .map_err(|_| XProtocolError::InternalServerError)?;
        Ok(HttpResponse::Ok().json(XResponse::new(StatusCode::OK, dup)))
    }

    pub async fn banner(data: web::Data<AppData>) -> Result<HttpResponse, XProtocolError> {
        if let Ok(banners) = data.banners().await {
            Ok(HttpResponse::Ok().json(XResponse::new(StatusCode::OK, banners)))
        } else {
            Ok(HttpResponse::Ok().json(XResponse::new(StatusCode::NOT_FOUND, " ")))
        }
    }

    pub async fn original(
        req: HttpRequest,
        info: web::Query<Pagination>,
        data: web::Data<AppData>,
        path: web::Path<ProposalAduitState>,
    ) -> Result<HttpResponse, XProtocolError> {
        let store = &data.store;
        let (query, page, count) = Self::get_common(req.clone(), info)?;
        let key = "token";
        let mut combine = CombineMap::new();
        let audit_state = path.into_inner();

        Self::insert_combine(
            &mut combine,
            "=",
            "audit_state".into(),
            format!("{:?}", audit_state),
        );
        
        if let Some(token) = Self::get_option::<String>(&query, key) {
            let token = token.to_lowercase();
            Self::insert_combine(&mut combine, "=", key.into(), token);
            
        }
        let order_map = vec![("proposal_id", true)];
        let dup = Self::about_me(store, &query).await?;
        let (total, list) = store
            .read_list(count, page, &combine, &order_map, dup)
            .await
            .map_err(|_| XProtocolError::Unknown)?;
        let list = list
            .iter()
            .map(|(proposal_id, create_time, address, _)| ProposalItem {
                proposal_id: *proposal_id,
                create_time: *create_time,
                address: address.clone(),
            })
            .collect::<Vec<ProposalItem>>();
        Ok(HttpResponse::Ok().json(XResponse::new(
            StatusCode::OK,
            ProposalList {
                current: page,
                total,
                list,
            },
        )))
    }

    pub async fn select_proposal(
        req: HttpRequest,
        path: web::Path<ProposalState>,
        info: web::Query<Pagination>,
        data: web::Data<AppData>,
    ) -> Result<HttpResponse, XProtocolError> {
        let store = &data.store;
        let status = path.into_inner();
        let now = Self::now();
        let (query, page, count) = Self::get_common(req, info)?;
        let key = "token";
        let mut combine = CombineMap::new();
        if let Some(token) = Self::get_option::<String>(&query, key) {
            let token = token.to_lowercase();
            Self::insert_combine(&mut combine, "=", key.into(), token);
        }
        let mut order_map = vec![("proposal_id", true)];
        let dup = match status {
            ProposalState::Formal => {
                Self::category_filter(&query, &mut combine);
                Self::insert_combine(&mut combine, ">", "close_time".into(), now.to_string());
                Self::liquidity_filter(&query, &mut order_map);
                Self::about_me(store, &query).await?
            }
            ProposalState::End /* | ProposalState::Referendum */ => {
                Self::category_filter(&query, &mut combine);
                Self::liquidity_filter(&query, &mut order_map);
                Self::about_me(store, &query).await?
            }
            _ => return Err(XProtocolError::MethodNotAllowed),
        };
        Self::insert_combine(&mut combine, "=", "state".into(), format!("{:?}", status));
        let (total, list) = store
            .read_list(count, page, &combine, &order_map, dup)
            .await
            .map_err(|_| XProtocolError::Unknown)?;
        let list = list
            .iter()
            .map(|(proposal_id, create_time, address, _)| ProposalItem {
                proposal_id: *proposal_id,
                create_time: *create_time,
                address: address.clone(),
            })
            .collect::<Vec<ProposalItem>>();
        Ok(HttpResponse::Ok().json(XResponse::new(
            StatusCode::OK,
            ProposalList {
                current: page,
                total,
                list,
            },
        )))
    }

    pub async fn index(
        req: HttpRequest,
        stream: web::Payload,
        data: web::Data<AppData>,
    ) -> Result<HttpResponse, XProtocolError> {
        ws::start(XProtocolWs::new((*data).clone()), &req, stream).map_err(|_| {
            XProtocolError::BadRequest
        })
    }

    pub async fn coins_support(data: web::Data<AppData>) -> Result<HttpResponse, XProtocolError> {
        let list = data
            .coins_support()
            .await
            .map_err(|_| XProtocolError::NotFound)?
            .iter()
            .map(|v| Coins {
                address: v.0.clone(),
                symbol: v.1.clone(),
            })
            .collect::<Vec<Coins>>();
        Ok(HttpResponse::Ok().json(XResponse::new(StatusCode::OK, list)))
    }

    pub async fn categories_support(
        data: web::Data<AppData>,
        req: HttpRequest,
    ) -> Result<HttpResponse, XProtocolError> {
        let filter = req
            .match_info()
            .get("filter")
            .ok_or(XProtocolError::ExpectationFailed)?;
        match filter{
            "categories" => {
                if let Ok(categories) = data.categories.read() {
                    let categories = categories.clone();
                    Ok(HttpResponse::Ok().json(XResponse::new(StatusCode::OK, categories)))
                } else {
                    Err(XProtocolError::InternalServerError)
                }
            }
            "liquidity" => {
                if let Ok(liquidity) = data.liquidity.read() {
                    let liquidity = liquidity.clone();
                    Ok(HttpResponse::Ok().json(XResponse::new(StatusCode::OK, liquidity)))
                } else {
                    Err(XProtocolError::InternalServerError)
                }
            }
            _ => Err(XProtocolError::ExpectationFailed),
        }
        
    }

    fn get_common(
        req: HttpRequest,
        info: web::Query<Pagination>,
    ) -> Result<(HashMap<String, String>, usize, usize), XProtocolError> {
        let query = web::Query::<HashMap<String, String>>::from_query(req.query_string())
            .map_err(|_| XProtocolError::ExpectationFailed)?
            .into_inner();
        let pagination = info.into_inner();
        let origin_page = pagination.page;
        let count = pagination.count;
        Ok((query, origin_page, count))
    }

    fn get_option<T>(map: &HashMap<String, String>, key: &'static str) -> Option<T>
    where
        T: FromStr,
    {
        if let Some(v) = map.get(key) {
            if v.is_empty() {
                None
            } else {
                v.parse::<T>().ok()
            }
        } else {
            None
        }
    }

    fn insert_combine(combine: &mut CombineMap, op: &'static str, key: String, value: String) {
        combine
            .entry(op) //空的或者被占用的
            .or_insert_with(HashMap::new) //空的就输入默认值,并返回当前值
            .insert(key, value); //返回值修改
    }

    fn category_filter(query: &HashMap<String, String>, combine: &mut CombineMap) {
        let key = "category";
        if let Ok(category) = Self::get_value::<usize>(query, key) {
            let key = key.to_string();
            Self::insert_combine(combine, "=", key, category.to_string());
        }
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn get_value<T>(map: &HashMap<String, String>, key: &'static str) -> Result<T, XProtocolError>
    where
        T: FromStr,
    {
        map.get(key)
            .ok_or(XProtocolError::ExpectationFailed)?
            .parse::<T>()
            .map_err(|_| XProtocolError::InsufficientStorage)
    }

    fn liquidity_filter(query: &HashMap<String, String>, order: &mut Vec<(&str, bool)>) {
        let key = "liquidity";
        let keys = ["volume24", "totalvolume", "highliquidity"];
        if let Ok(liquidity) = Self::get_value::<usize>(query, key) {
            if let Some(&key) = keys.get(liquidity) {
                Self::insert_order_map(order, key, false);
            } //0 1 2 3
            if liquidity == 5 {
                Self::insert_order_map(order, "close_time", true);
            }
            if liquidity == 4 {
                Self::insert_order_map(order, "proposal_id", false);
            } else {
                Self::insert_order_map(order, "proposal_id", true); //空
            }
        }
    }

    fn insert_order_map(order: &mut Vec<(&str, bool)>, key: &'static str, value: bool) {
        order.retain(|&v| v.0 != key);
        order.push((key, value));
    }

    async fn about_me(
        store: &StoreDB,
        query: &HashMap<String, String>,
    ) -> Result<Option<Vec<u64>>, XProtocolError> {
        let key = "aboutMe";
        if let Ok(relation) = Self::get_value::<u32>(query, key) {
            let account = Self::get_value::<String>(query, "account")?;
            let account = account.to_lowercase();
            let relation = format!("{:?}", TryInto::<ProposalRelation>::try_into(relation)?);
            log::error!("relation {:?}",relation);
            let list = store.read_relation(account, relation).await.map_err(|_| {
                XProtocolError::InternalServerError
            })?;
            Ok(if list.is_empty() {
                Some(vec![0])
            } else {
                Some(list)
            })
        } else {
            Ok(None)
        }
    }
}
