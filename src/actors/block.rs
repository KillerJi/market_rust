use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};

use actix::{fut, Actor, AsyncContext, Context, Handler, Message, Recipient};
use actix_web::rt::time as RuntimeTime;
use web3::{
    transports,
    types::{FilterBuilder, H160},
    Web3,
};

use crate::{
    datas::{config::ContractConfig, data::AppData, BoxedResult},
    xprotocol::{factory::Factory, proposals::Proposal, router::Router, ModuleTest},
};

use super::ws::SubOpCode;

#[derive(Message)]
#[rtype("()")]
pub struct Ping(pub u64);

#[derive(Clone)]
pub struct BlockActor {
    data: Arc<AppData>,
    web3: Arc<Web3<transports::Http>>,
    from_block: Arc<AtomicU64>,
    start: Arc<AtomicU64>,
    rec_ping: Option<Recipient<Ping>>,
    module_test: Arc<HashMap<H160, Box<dyn ModuleTest>>>,
}

impl BlockActor {
    pub fn new(data: Arc<AppData>, contract: &ContractConfig) -> BoxedResult<Self> {
        let transport = transports::Http::new(&contract.rpc)?;
        let web3 = Arc::new(Web3::new(transport));

        let factory = contract.factory.parse::<H160>()?;
        let router = contract.router.parse::<H160>()?;
        let mut module_test = HashMap::<H160, Box<dyn ModuleTest>>::new();
        module_test.insert(factory, Box::new(Factory::new(data.clone(), web3.clone())));
        module_test.insert(router, Box::new(Router::new(data.clone(), web3.clone())));
        let obj = Self {
            data,
            web3,
            from_block: Arc::new(AtomicU64::new(0)),
            start: Arc::new(AtomicU64::new(0)),
            rec_ping: None,
            module_test: Arc::new(module_test),
        };
        Ok(obj)
    }

    /*
        更新data的block
        获取data存储的block,如果不等于最新的block,则更新
    */
    fn update_data_block(&self, block: u64) {
        let current_block = self.data.get_current_block();
        if current_block != block {
            self.data
                .push_to_client("newBlock", SubOpCode::Update, block);
            self.data.set_current_block(block);
        }
    }

    /*
        获取链最新block,更新data的block
    */
    pub async fn tick(&self, block_step: u64) -> BoxedResult<()> {
        let web3 = self.web3.clone();

        let block = web3.eth().block_number().await?.as_u64();
        self.update_data_block(block);

        let from_block = self.from_block.load(Ordering::Relaxed);
        let mut to_block = from_block + block_step - 1; //from 0 100 99  1000
        if to_block >= block {
            to_block = block;
        }
        
        log::info!("from {} to {}, total {}", from_block, to_block, block);

        if from_block >= to_block {
            // from是最新区块，检查热度1小时
            self.normal_update(block).await
        } else {
            //from不是最新区块，更新合约日志，检查热度
            self.log_update(from_block, to_block, block).await
        }
    }

    async fn normal_update(&self, block: u64) -> BoxedResult<()> {
        let web3 = self.web3.clone();
        let data = self.data.clone();

        let addrs = self.data.get_proposals(); //获得所有提案ID
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();
        if self.start.load(Ordering::Relaxed) + 60 * 60 < now {
            Proposal::update_24h_hot(&data, &web3, addrs, block)
                .await
                .map_err(|e| e.to_string())?;
            self.start.swap(now, Ordering::Relaxed);
        }
        RuntimeTime::sleep(Duration::from_secs(10)).await;
        Ok(())
    }

    async fn log_update(&self, from_block: u64, to_block: u64, block: u64) -> BoxedResult<()> {
        let web3 = self.web3.clone();
        let data = self.data.clone();
        let store = &data.store;

        let mut contracts = self.module_test.keys().cloned().collect::<Vec<H160>>();
        let addrs = self.data.get_proposals();
        contracts.extend(addrs.iter());
        let filter = FilterBuilder::default()
            .from_block(from_block.into())
            .to_block(to_block.into())
            .address(contracts.clone())
            .build();

        let logs = web3.eth().logs(filter).await?;

        let mut addrs = Vec::<H160>::new();
        for log in logs.iter() {
            //如果日志中 router和factory状态变化 则执行更新
            if let Some(without_data) = self.module_test.get(&log.address) {
                if let Err(e) = without_data.with_fn(log).await {
                    log::error!("test with {:?} error: {:?}", log.address, e);
                }
            }
            //如果日志中有提案状态变化 则执行更新
            if data.contains_proposal(&log.address) {
                if let Err(e) = Proposal::with_proposal(&self.data, &web3, log).await {
                    log::error!("with proposal error: {:?}", e);
                }
                addrs.push(log.address);
            }
        }
        //更新日志中产生变化的提案hot
        Proposal::update_24h_hot(&data, &web3, addrs, block)
            .await
            .map_err(|e| e.to_string())?;

        //更新数据库的检索区块
        let chain_id = self.data.chain_id;
        if let Err(e) = store.write_block_hight(chain_id, to_block).await {
            log::error!("write block hight error: {:?}", e);
        }
        //更新block下一次检索区块
        self.from_block.swap(to_block + 1, Ordering::Relaxed);
        Ok(())
    }

    /*
        读取数据库的from_block step,更新block的from_block，调用send_ping
    */
    async fn async_started(&self) {
        let store = &self.data.store;
        let chain_id = self.data.chain_id;
        let (from_block, block_step) = store.read_block(chain_id).await.unwrap_or((0, 100));
        self.from_block.swap(from_block, Ordering::Relaxed);
        // Proposal::update_total(&self.data, &self.web3, from_block).await;
        self.send_ping(block_step);
    }

    /*
        获取recipient地址,do_send发送step,触发handle
    */
    fn send_ping(&self, block_step: u64) {
        if let Some(rec) = &self.rec_ping {
            if let Err(e) = rec.do_send(Ping(block_step)) {
                log::error!("send ping with error: {:?}", e);
            }
        }
    }
}

impl Actor for BlockActor {
    type Context = Context<Self>;

    /*
        blockactor启动,调用async_started
    */
    fn started(&mut self, ctx: &mut Self::Context) {
        let add = ctx.address();
        self.rec_ping = Some(add.recipient());
        let shadow_self = self.clone();
        super::async_call(
            self,
            ctx,
            async move { shadow_self.async_started().await },
            |_, _, _| fut::ready(()),
        );
    }
}

impl Handler<Ping> for BlockActor {
    type Result = ();

    /*
        调用tick
    */
    fn handle(&mut self, msg: Ping, ctx: &mut Self::Context) -> Self::Result {
        let shadow = self.clone();
        let step = msg.0;
        super::async_call(
            self,
            ctx,
            async move { shadow.tick(step).await },
            move |r, a, _| {
                if let Err(e) = r {
                    log::error!("handle ping error: {:?}", e);
                }
                a.send_ping(step);
                fut::ready(())
            },
        );
    }
}
