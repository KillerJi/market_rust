use crate::{
    datas::handle::ErrorResponse,
    datas::{data::AppData, BoxedResult},
};
use actix::{fut, Actor, Handler, Message, StreamHandler, AsyncContext, ActorContext};
use actix_web::http::StatusCode;
use actix_web_actors::ws;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum OpCode {
    Ping,
    Pong,
    Sub,
    Unsub,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct XWsSub {
    pub target: String,
    pub id: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct XWsSubRes {
    pub code: u16,
    pub id: u64,
}

impl XWsSubRes {
    pub fn new(code: StatusCode, id: u64) -> Self {
        Self {
            code: code.as_u16(),
            id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SubOpCode {
    Add,
    Del,
    Update,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubOp<T> {
    pub op: SubOpCode,
    pub target: String,
    pub data: T,
    pub id: u64,
}

impl<T> SubOp<T> {
    pub fn new(op: SubOpCode, target: String, data: T, id: u64) -> Self {
        Self {
            op,
            target,
            data,
            id,
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct WsMessage(pub String);

#[derive(Clone)]
pub struct XProtocolWs {
    data: Arc<AppData>,
    hb: Arc<RwLock<Instant>>,
}

impl XProtocolWs {
    pub fn new(data: Arc<AppData>) -> Self {
        Self {
            data,
            hb: Arc::new(RwLock::new(Instant::now())),
        }
    }

    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if let Ok(hb) = act.hb.read() {
                if Instant::now().duration_since(*hb) > CLIENT_TIMEOUT {
                    log::info!("Websocket Client heartbeat failed, disconnecting!");
                    ctx.stop();
                }
            }
        });
    }

    fn get_json_value<T>(text: &str, key: &str) -> BoxedResult<T>
    where
        T: DeserializeOwned,
    {
        let req: Value = serde_json::from_str(text)?;
        let target_value = req.get(key).ok_or(format!("{} key not found", key))?;
        serde_json::from_value::<T>(target_value.clone()).map_err(|e| e.into())
    }

    fn with_text(&mut self, ctx: &mut <Self as Actor>::Context, text: String) {
        let shadow_self = self.clone();
        let addr = ctx.address();
        super::async_call(
            self,
            ctx,
            async move {
                let op_key = "op";
                let op = Self::get_json_value::<OpCode>(&text, op_key)?;
                match op {
                    OpCode::Ping => {
                        if let Ok(mut hb) = shadow_self.hb.write() {
                            *hb = Instant::now();
                        }
                        let v = json!({ op_key: OpCode::Pong });
                        serde_json::to_string(&v).map_err(|e| e.into())
                    }
                    OpCode::Pong => {
                        if let Ok(mut hb) = shadow_self.hb.write() {
                            *hb = Instant::now();
                        }
                        let v = json!({ op_key: OpCode::Ping });
                        serde_json::to_string(&v).map_err(|e| e.into())
                    }
                    OpCode::Sub => {
                        let sub = serde_json::from_str::<XWsSub>(&text)?;
                        shadow_self.data.client_sub(addr.recipient(), sub.clone())?;
                        serde_json::to_string(&XWsSubRes::new(StatusCode::OK, sub.id))
                            .map_err(|e| e.into())
                    }
                    OpCode::Unsub => {
                        let sub = serde_json::from_str::<XWsSub>(&text)?;
                        shadow_self.data.client_unsub(addr.recipient(), &sub)?;
                        serde_json::to_string(&XWsSubRes::new(StatusCode::OK, sub.id))
                            .map_err(|e| e.into())
                    }
                }
            },
            |r: Result<String, Box<dyn std::error::Error>>, _, c| {
                c.text(r.unwrap_or_else(|e| {
                    serde_json::to_string(&ErrorResponse::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        e.to_string(),
                    ))
                    .unwrap_or_else(|e| e.to_string())
                }));
                fut::ready(())
            },
        );
    }
}

impl Actor for XProtocolWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        self.hb(ctx);
    }

    fn stopped(&mut self, ctx: &mut <Self as Actor>::Context) {
        let addr = ctx.address();
        self.data.delete_client(&addr.recipient());
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for XProtocolWs {
    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut <Self as Actor>::Context,
    ) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                if let Ok(mut hb) = self.hb.write() {
                    *hb = Instant::now();
                }
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(msg)) => {
                if let Ok(mut hb) = self.hb.write() {
                    *hb = Instant::now();
                }
                ctx.ping(&msg);
            }
            Ok(ws::Message::Text(text)) => {
                self.with_text(ctx, (*text).to_string());
            }
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => {
                self.data.delete_client(&ctx.address().recipient());
                ctx.close(reason);
                ctx.stop();
            }
            _ => (),
        }
    }
}

impl Handler<WsMessage> for XProtocolWs {
    type Result = ();

    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}
