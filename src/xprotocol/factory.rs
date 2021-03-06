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

    //???????????????????????????
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

        self.data //??????data
            .insert_support(addr.clone(), symbol.clone(), flag)
            .map_err(|e| e.to_string())?;

        if let Err(e) = self //??????db
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

    //factory????????????
    async fn with_factory(&self, log: &Log) -> BoxedSyncResult<()> {
        //??????raw_log
        let raw_log = ethabi::RawLog {
            topics: log.topics.clone(),
            data: log.data.0.clone(),
        };
        //??????abi??????contract
        let contract = ethabi::Contract::load(include_bytes!("../res/factory_abi.json").as_ref())?;
        //??????????????????????????????
        let box_fns = Self::get_support_test_fn();
        for (flag, test_fn) in box_fns.iter() {
            //??????????????????
            if let Ok(addr) = test_fn(&raw_log, &contract) {
                if let Err(e) = self.with_address(addr, *flag).await {
                    //???????????????
                    log::error!("with address error: {:?}", e);
                }
            }
        }
        //
        //?????????????????????event?????????  ??????ID ???????????? ????????????
        if let Ok((proposal_id, proposal_add, create_time)) =
            Self::parse_log_create_proposal(&raw_log, &contract)
        {
            //????????????hash?????????????????????
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
        ??????factory ?????????????????????
        ?????????????????????????????????event
        ???????????????proposal,relation,price
        ??????actix data
    */
    async fn update_new_proposal(
        &self,
        contract: &ethabi::Contract,
        transaction_id: TransactionId,
        proposal_id: u64,
        proposal_add: H160,
        create_time: u64,
    ) -> BoxedSyncResult<()> {
        // ???????????????
        let store = &self.data.store;
        //?????????????????????event?????????  ??????ID ???????????? ????????????
        // let (proposal_id, proposal_add, create_time) =
        //     Self::parse_log_create_proposal(raw_log, contract)?;
        //????????????
        let transaction = self
            .web3
            .eth()
            .transaction(transaction_id)
            .await?
            .ok_or("get transaction return nill")?;
        //????????????????????????    ??????????????????  ????????????  ?????????????????? ?????????????????????
        let (close_time, category, token, number) =
            Self::parse_create_proposal_input(contract, &transaction.input.0[4..])?;
        //??????proposal
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
        // ??????relation
        store
            .write_relation(proposal_id, owner, relation)
            .await
            .map_err(|e| e.to_string())?;
        // ??????price
        store
            .write_price(proposal_id, create_time, [50, 50])
            .await
            .map_err(|e| e.to_string())?;
        // data??????
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
