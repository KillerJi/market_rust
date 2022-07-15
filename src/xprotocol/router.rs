use std::{collections::HashMap, sync::Arc};

use futures::{future::LocalBoxFuture, FutureExt};
use web3::{
    ethabi, transports,
    types::{Log, TransactionId, H160},
    Web3,
};

use crate::{
    actors::ws::SubOpCode,
    datas::{
        data::AppData,
        handle::{ProposalAduitState, ProposalState},
        BoxedSyncResult,
    },
};

use super::{BoxFn, ModuleTest, ProposalStatus, XProtocol};

pub struct Router {
    data: Arc<AppData>,
    web3: Arc<Web3<transports::Http>>,
}

impl Router {
    pub fn new(data: Arc<AppData>, web3: Arc<Web3<transports::Http>>) -> Self {
        Self { data, web3 }
    }

    fn parse_proposal_id(
        raw_log: &ethabi::RawLog,
        contract: &ethabi::Contract,
        name: &str,
    ) -> BoxedSyncResult<u64> {
        // event AcceptStateFormalPrediction(uint proposalId);
        // event AcceptProposalEnd (uint proposalId);
        let abi_log = XProtocol::parse_log(raw_log, contract, name)?;
        let params = abi_log.params;

        XProtocol::get_index(&params, 0)?
            .value
            .into_uint()
            .map(|v| v.as_u64())
            .ok_or_else(|| "convert to uint error".into())
    }

    fn get_state_test_fn() -> HashMap<ProposalState, BoxFn<u64>> {
        let mut box_fns = HashMap::<ProposalState, BoxFn<u64>>::new();
        box_fns.insert(
            ProposalState::Formal,
            Box::new(|raw_log, contract| {
                Self::parse_proposal_id(raw_log, contract, "AcceptStateFormalPrediction")
            }),
        );
        box_fns.insert(
            ProposalState::End,
            Box::new(|raw_log, contract| {
                Self::parse_proposal_id(raw_log, contract, "AcceptProposalEnd")
            }),
        );
        box_fns
    }

    async fn with_router(&self, log: &Log) -> BoxedSyncResult<()> {
        //构建raw_log
        let raw_log = ethabi::RawLog {
            topics: log.topics.clone(),
            data: log.data.0.clone(),
        };
        //根据abi构建contract
        let contract = ethabi::Contract::load(include_bytes!("../res/router_abi.json").as_ref())?;
        //根据交易hash更新提案状态
        self.update_proposal_state(
            &raw_log,
            &contract,
            TransactionId::Hash(log.transaction_hash.ok_or("txid is none")?),
        )
        .await
    }

    async fn update_proposal_state(
        &self,
        raw_log: &ethabi::RawLog,
        contract: &ethabi::Contract,
        transaction_id: TransactionId,
    ) -> BoxedSyncResult<()> {
        // 数据库写入
        let store = &self.data.store;
        // 读取交易hash中的审核操作
        let transaction = self
            .web3
            .eth()
            .transaction(transaction_id)
            .await?
            .ok_or("get transaction return nill")?;
        if let Ok((proposal_address, audit_state)) = Self::parse_factory_function(
            contract,
            "acceptStateFormalPrediction",
            &transaction.input.0[4..],
        ) {
            match audit_state {
                false => {
                    //不通过 End 地址
                    let account = format!("{:?}", proposal_address);
                    let audit_state = format!("{:?}", ProposalAduitState::NotPassed);
                    if let Err(e) = store.write_proposal_audit_state(account, audit_state).await {
                        log::error!("write proposal state error: {:?}", e);
                    }
                }
                true => {
                    //通过 formal 地址
                    let account = format!("{:?}", proposal_address);
                    let audit_state = format!("{:?}", ProposalAduitState::Passed);
                    if let Err(e) = store.write_proposal_audit_state(account, audit_state).await {
                        log::error!("write proposal state error: {:?}", e);
                    }
                }
            }
        }
        //读取event 修改数据库的提案状态
        let box_fns = Self::get_state_test_fn();
        for (&state, test_fn) in box_fns.iter() {
            if let Ok(proposal_id) = test_fn(&raw_log, &contract) {
                self.data.set_proposal_state(proposal_id, state);
                if let Some(address) = self.data.get_proposal_address(proposal_id) {
                    self.data.push_to_client(
                        "proposalStatus",
                        SubOpCode::Update,
                        ProposalStatus::new(proposal_id, address, state),
                    );
                }
                let proposal_id = format!("{:?}", proposal_id);
                let state = format!("{:?}", state);
                if let Err(e) = store.write_proposal_state(proposal_id, state).await {
                    log::error!("write proposal state error: {:?}", e);
                }
                break;
            }
        }
        Ok(())
    }

    fn parse_factory_function(
        contract: &ethabi::Contract,
        name: &str,
        input: &[u8],
    ) -> BoxedSyncResult<(H160, bool)> {
        let function = contract.function(name)?;
        let params = function.decode_input(input)?;
        let proposal_address = XProtocol::get_index(&params, 0)?
            .into_address()
            .ok_or("into address error")?;
        let params2 = XProtocol::get_index(&params, 1)?
            .into_bool()
            .ok_or("into uint error")?;
        Ok((proposal_address, params2))
    }
}

impl ModuleTest for Router {
    fn with_fn<'a>(&'a self, log: &'a Log) -> LocalBoxFuture<'a, BoxedSyncResult<()>> {
        self.with_router(log).boxed()
    }
}
