use std::{collections::HashMap, sync::Arc};

use web3::{
    contract::{Contract, Options},
    ethabi::{self, Uint},
    transports,
    types::{BlockId, FilterBuilder, Log, H160, U256},
    Web3,
};

use crate::datas::{data::AppData, handle::ProposalRelation, BoxedSyncResult};

use super::XProtocol;

pub struct Proposal;

impl Proposal {
    pub async fn update_24h_hot(
        data: &Arc<AppData>,
        web3: &Arc<Web3<transports::Http>>,
        addrs: Vec<H160>,
        hight: u64,
    ) -> BoxedSyncResult<()> {
        if addrs.is_empty() {
            return Ok(());
        }
        let store = &data.store;
        //获取起点区块
        let from_24h = if hight < (24 * 60 * 60 / 15) {
            0
        } else {
            hight - (24 * 60 * 60 / 15)
        };
        //获取所有提案的24小时交易额
        if let Ok(total) = Self::with_volume(web3, from_24h, hight, addrs.clone()).await {
            for (addr, total) in total.iter() {
                if let Some(proposal_id) = data.get_proposal_id(addr) {
                    let volume = format!("{}", total);
                    //写入交易额到数据库中
                    if let Err(e) = store.write_volume24(proposal_id, volume).await {
                        log::error!("{:?}", e);
                    }
                }
            }
        }
        Ok(())
    }

    /*
        获取24h内的提案总交易额
        根据add数组里的提案add,分别计算买卖总额,并返回所有提案add和对应的交易额
    */
    async fn with_volume(
        web3: &Arc<Web3<transports::Http>>,
        from_block: u64,
        to_block: u64,
        addrs: Vec<H160>,
    ) -> BoxedSyncResult<HashMap<H160, u128>> {
        let mut total = HashMap::<H160, u128>::new();
        if addrs.is_empty() {
            return Ok(total);
        }
        let filter = FilterBuilder::default()
            .from_block(from_block.into())
            .to_block(to_block.into())
            .address(addrs)
            .build();
        let logs = web3.eth().logs(filter).await?;
        let contract = ethabi::Contract::load(include_bytes!("../res/proposal_abi.json").as_ref())?;
        for log in logs.iter() {
            let raw_log = ethabi::RawLog {
                topics: log.topics.clone(),
                data: log.data.0.clone(),
            };
            let mut sum = 0;
            for name in ["Buy", "Sell"] {
                if let Ok(amount) = Self::parse_amount(&raw_log, &contract, name) {
                    sum += amount;
                }
            }

            total
                .entry(log.address)
                .and_modify(|v| *v += sum)
                .or_insert(sum);
        }
        Ok(total)
    }
    //解析日志里的买卖交易额，并返回
    fn parse_amount(
        raw_log: &ethabi::RawLog,
        contract: &ethabi::Contract,
        name: &str,
    ) -> BoxedSyncResult<u128> {
        // event Buy(uint tokenIndex, address account, uint256 amount);
        // event Sell(uint tokenIndex, address account, uint256 amount);
        let abi_log = XProtocol::parse_log(raw_log, contract, name)?;
        XProtocol::get_index(&abi_log.params, 2)?
            .value
            .into_uint()
            .map(|v| v.as_u128())
            .ok_or_else(|| "convert to address error".into())
    }

    fn parse_trade(
        raw_log: &ethabi::RawLog,
        contract: &ethabi::Contract,
        name: &str,
    ) -> BoxedSyncResult<H160> {
        // event Buy(address token, address account, uint256 amount);
        // event Sell(address token, address account, uint256 amount);

        let abi_log = XProtocol::parse_log(raw_log, contract, name)?;
        let params = abi_log.params;

        let account = XProtocol::get_index(&params, 1)?
            .value
            .into_address()
            .ok_or("convert to address error")?;

        Ok(account)
    }

    fn parse_liquidity(
        raw_log: &ethabi::RawLog,
        contract: &ethabi::Contract,
        name: &str,
    ) -> BoxedSyncResult<H160> {
        // event AddLiquidity(address account, uint256 amount, uint256 proposalId);
        // event RemoveLiquidity(address account, uint256 amount, uint256 proposalId);

        let abi_log = XProtocol::parse_log(raw_log, contract, name)?;
        let params = abi_log.params;

        let account = XProtocol::get_index(&params, 0)?
            .value
            .into_address()
            .ok_or("convert to address error")?;

        Ok(account)
    }

    pub async fn with_proposal(
        data: &Arc<AppData>,
        web3: &Arc<Web3<transports::Http>>,
        log: &Log,
    ) -> BoxedSyncResult<()> {
        //获取proposalid
        let proposal_id = data
            .get_proposal_id(&log.address)
            .ok_or("proposal not exist")?;
        //构建raw_log
        let raw_log = ethabi::RawLog {
            topics: log.topics.clone(),
            data: log.data.0.clone(),
        };
        //根据abi构建contract
        let proposal = Contract::from_json(
            web3.eth(),
            log.address,
            include_bytes!("../res/proposal_abi.json"),
        )?;
        let (volume_falg, liquidity_flag, price_flag) =
            Self::update_volumeand_relation(data, &raw_log, proposal.abi(), proposal_id).await;
        // 交易额更新
        if volume_falg {
            Self::update_history(data, web3, log, &proposal, proposal_id, 1).await?;
        }
        // 流动性更新
        if liquidity_flag {
            Self::update_history(data, web3, log, &proposal, proposal_id, 2).await?;
        }
        // 价格更新
        if price_flag {
            Self::update_history(data, web3, log, &proposal, proposal_id, 3).await?;
        }
        Ok(())
    }
    //检索到事件 更新flag则为true
    async fn update_volumeand_relation(
        data: &Arc<AppData>,
        raw_log: &ethabi::RawLog,
        contract: &ethabi::Contract,
        proposal_id: u64,
    ) -> (bool, bool, bool) {
        let store = &data.store;
        let mut volume_need_update = false;
        let mut liquidity_need_update = false;
        let mut price_need_update = false;
        //读取日志中的Buy Sell
        let relation = format!("{:?}", ProposalRelation::Trade);
        for name in ["Buy", "Sell"] {
            if let Ok(account) = Self::parse_trade(raw_log, contract, name) {
                let account = format!("{:?}", account);
                // 更新relation trade
                if let Err(e) = store
                    .write_relation(proposal_id, account, relation.clone())
                    .await
                {
                    log::error!("{:?}", e);
                }
                volume_need_update = true;
                price_need_update = true;
            }
        }
        let relation = format!("{:?}", ProposalRelation::Liquidity);
        for name in ["AddLiquidity", "RemoveLiquidity"] {
            if let Ok(account) = Self::parse_liquidity(raw_log, contract, name) {
                let account = format!("{:?}", account);
                if let Err(e) = store
                    .write_relation(proposal_id, account, relation.clone())
                    .await
                {
                    log::error!("{:?}", e);
                }
                liquidity_need_update = true;
                price_need_update = true;
            }
        }
        (volume_need_update, liquidity_need_update, price_need_update)
    }

    async fn update_history(
        data: &Arc<AppData>,
        web3: &Arc<Web3<transports::Http>>,
        log: &Log,
        proposal: &Contract<transports::Http>,
        proposal_id: u64,
        way: u32,
    ) -> BoxedSyncResult<()> {
        let store = &data.store;
        let block_id = BlockId::Hash(log.block_hash.ok_or("block hash empty")?);

        match way {
            1 => {
                let total_volume: Uint = proposal
                    .query("totalVolume", (), None, Options::default(), block_id)
                    .await?;
                let volume = format!("{:?}", total_volume);
                data.store
                    .write_volume(proposal_id, volume)
                    .await
                    .map_err(|e| e.to_string().into())
            }
            2 => {
                let total_supply: Uint = proposal
                    .query("totalSupply", (), None, Options::default(), block_id)
                    .await?;
                let liquidity = format!("{:?}", total_supply);
                data.store
                    .write_liquidity(proposal_id, liquidity)
                    .await
                    .map_err(|e| e.to_string().into())
            }
            3 => {
                let block = web3.eth().block(block_id).await?.ok_or("empty block")?;
                let ts = block.timestamp.as_u64();
                let mut tokens = [0u128; 2];
                for (i, &token_name) in ["token0", "token1"].iter().enumerate() {
                    let token = proposal
                        .query(token_name, (), None, Options::default(), block_id)
                        .await?;
                    let contract = Contract::from_json(
                        web3.eth(),
                        token,
                        include_bytes!("../res/erc20_abi.json"),
                    )?;
                    let total: U256 = contract
                        .query(
                            "balanceOf",
                            (log.address,),
                            None,
                            Options::default(),
                            block_id,
                        )
                        .await?;
                    tokens[i] = total.as_u128();
                }
                tokens[0] = tokens[0] * 100 / (tokens[0] + tokens[1]);
                tokens[1] = 100 - tokens[0];
                store
                    .write_price(proposal_id, ts, tokens)
                    .await
                    .map_err(|e| e.to_string().into())
            }
            _ => Ok(()),
        }
    }
}
