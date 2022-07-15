use std::{collections::HashMap, sync::Arc};

use futures::{future::LocalBoxFuture, FutureExt};
use web3::{
    contract::{Contract, Options},
    ethabi, transports,
    types::{Log, TransactionId, H160},
    Web3,
};

use crate::{
    actors::ws::SubOpCode,
    datas::{
        data::AppData,
        handle::{ProposalRelation, ProposalState},
        BoxedSyncResult,
    },
    xprotocol::ProposalStatus,
};

use super::{BoxFn, ModuleTest, XProtocol};

pub struct Factory {
    data: Arc<AppData>,
    web3: Arc<Web3<transports::Http>>,
}

impl Factory {
    pub fn new(data: Arc<AppData>, web3: Arc<Web3<transports::Http>>) -> Self {
        Self { data, web3 }
    }

    fn parse_log_address(
        raw_log: &ethabi::RawLog,
        contract: &ethabi::Contract,
        name: &str,
    ) -> BoxedSyncResult<H160> {
        // event SettlementCurrencyPaused(address token);
        // event SupportMarkets(address token);

        let abi_log = XProtocol::parse_log(raw_log, contract, name)?;
        XProtocol::get_index(&abi_log.params, 0)?
            .value
            .into_address()
            .ok_or_else(|| "convert to address error".into())
    }

    fn parse_log_create_proposal(
        raw_log: &ethabi::RawLog,
        contract: &ethabi::Contract,
    ) -> BoxedSyncResult<(u64, H160, u64)> {
        // event CreateProposal(uint proposalId, address token0, address token1, address pair);

        let abi_log = XProtocol::parse_log(raw_log, contract, "CreateProposal")?;
        let params = abi_log.params;

        let proposal_id = XProtocol::get_index(&params, 0)?
            .value
            .into_uint()
            .ok_or("convert to uint error")?
            .as_u64();

        let proposal = XProtocol::get_index(&params, 3)?
            .value
            .into_address()
            .ok_or("convert to address error")?;

        let time = XProtocol::get_index(&params, 4)?
            .value
            .into_uint()
            .ok_or("convert to uint error")?
            .as_u64();
        Ok((proposal_id, proposal, time))
    }

    fn parse_create_proposal_input(
        contract: &ethabi::Contract,
        input: &[u8],
    ) -> BoxedSyncResult<(u64, u64, H160, u128)> {
        // function createProposal(string memory title, string memory details, string[2] memory outcome, uint closeTime, uint category,
        //     address foundMarket, uint256 amount, uint256 feeRatio)
        let function = contract.function("createProposal")?;

        let params = function.decode_input(input)?;
        let close_time = XProtocol::get_index(&params, 3)?
            .into_uint()
            .ok_or("into uint error")?
            .as_u64();

        let category = XProtocol::get_index(&params, 4)?
            .into_uint()
            .ok_or("into uint error")?
            .as_u64();

        let found = XProtocol::get_index(&params, 5)?
            .into_address()
            .ok_or("into address error")?;

        let number = XProtocol::get_index(&params, 6)?
            .into_uint()
            .ok_or("into uint error")?
            .as_u128();
        Ok((close_time, category, found, number))
    }

    //修改数据库支持币种
    async fn with_address(&self, addr: H160, flag: bool) -> BoxedSyncResult<()> {
        let erc20 = Contract::from_json(
            self.web3.eth(),
            addr,
            include_bytes!("../res/erc20_abi.json"),
        )?;
        let addr = format!("{:?}", addr);
        let symbol: String = erc20
            .query("symbol", (), None, Options::default(), None)
            .await?;

        self.data //写入data
            .insert_support(addr.clone(), symbol.clone(), flag)
            .map_err(|e| e.to_string())?;

        if let Err(e) = self //写入db
            .data
            .store
            .write_coins_support(addr.clone(), symbol, flag)
            .await
        {
            log::error!("write coins support error: {:?}", e);
        }
        self.data.push_to_client(
            "coinsSupport",
            [SubOpCode::Del, SubOpCode::Add][flag as usize],
            addr,
        );
        Ok(())
    }

    fn get_support_test_fn() -> HashMap<bool, BoxFn<H160>> {
        let mut box_fns = HashMap::<bool, BoxFn<H160>>::new();
        box_fns.insert(
            true,
            Box::new(|raw_log, contract| {
                Self::parse_log_address(raw_log, contract, "SupportMarkets")
            }),
        );
        box_fns.insert(
            false,
            Box::new(|raw_log, contract| {
                Self::parse_log_address(raw_log, contract, "SettlementCurrencyPaused")
            }),
        );
        box_fns
    }

    //factory日志更新
    async fn with_factory(&self, log: &Log) -> BoxedSyncResult<()> {
        //构建raw_log
        let raw_log = ethabi::RawLog {
            topics: log.topics.clone(),
            data: log.data.0.clone(),
        };
        //根据abi构建contract
        let contract = ethabi::Contract::load(include_bytes!("../res/factory_abi.json").as_ref())?;
        //更新支持和不支持币种
        let box_fns = Self::get_support_test_fn();
        for (flag, test_fn) in box_fns.iter() {
            //修改支持币种
            if let Ok(addr) = test_fn(&raw_log, &contract) {
                if let Err(e) = self.with_address(addr, *flag).await {
                    //写入或修改
                    log::error!("with address error: {:?}", e);
                }
            }
        }
        //
        //日志解析新提案event的信息  提案ID 提案地址 创建时间
        if let Ok((proposal_id, proposal_add, create_time)) =
            Self::parse_log_create_proposal(&raw_log, &contract)
        {
            //读取交易hash，获取创建提案
            self.update_new_proposal(
                &contract,
                TransactionId::Hash(log.transaction_hash.ok_or("txid is none")?),
                proposal_id,
                proposal_add,
                create_time,
            )
            .await
        } else {
            Ok(())
        }
    }

    /*
        读取factory 某一区块的日志
        获取创建提案输入参数和event
        写入数据库proposal,relation,price
        写入actix data
    */
    async fn update_new_proposal(
        &self,
        contract: &ethabi::Contract,
        transaction_id: TransactionId,
        proposal_id: u64,
        proposal_add: H160,
        create_time: u64,
    ) -> BoxedSyncResult<()> {
        // 数据库写入
        let store = &self.data.store;
        //日志解析新提案event的信息  提案ID 提案地址 创建时间
        // let (proposal_id, proposal_add, create_time) =
        //     Self::parse_log_create_proposal(raw_log, contract)?;
        //获取交易
        let transaction = self
            .web3
            .eth()
            .transaction(transaction_id)
            .await?
            .ok_or("get transaction return nill")?;
        //解析交易输入数据    提案结束时间  提案类别  结算币种地址 初始流动性数量
        let (close_time, category, token, number) =
            Self::parse_create_proposal_input(contract, &transaction.input.0[4..])?;
        //写入proposal
        let state = format!("{:?}", ProposalState::Original);
        store
            .write_proposals(
                proposal_id,
                format!("{:?}", proposal_add),
                category,
                format!("{:?}", token),
                state,
                number,
                [create_time, close_time],
            )
            .await
            .map_err(|e| e.to_string())?;

        let owner = format!("{:?}", transaction.from.ok_or("from address none")?);
        let relation = format!("{:?}", ProposalRelation::Create);
        // 写入relation
        store
            .write_relation(proposal_id, owner, relation)
            .await
            .map_err(|e| e.to_string())?;
        // 写入price
        store
            .write_price(proposal_id, create_time, [50, 50])
            .await
            .map_err(|e| e.to_string())?;
        // data写入
        self.data.insert_proposal(proposal_add, proposal_id);
        self.data
            .set_proposal_state(proposal_id, ProposalState::Original);
        self.data.push_to_client(
            "proposalStatus",
            SubOpCode::Update,
            ProposalStatus::new(proposal_id, proposal_add, ProposalState::Original),
        );
        Ok(())
    }
}

impl ModuleTest for Factory {
    fn with_fn<'a>(&'a self, log: &'a Log) -> LocalBoxFuture<'a, BoxedSyncResult<()>> {
        self.with_factory(log).boxed()
    }
}
