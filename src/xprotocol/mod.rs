pub(crate) mod factory;
pub(crate) mod proposals;
pub(crate) mod router;

use futures::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};
use web3::{ethabi, types::{Log, H160}};

use crate::datas::{BoxedSyncResult, handle::ProposalState};

pub type BoxFn<T> =
    Box<dyn Fn(&ethabi::RawLog, &ethabi::Contract) -> BoxedSyncResult<T> + Send + Sync>;
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProposalStatus {
    pub proposal_id: u64,
    pub address: String,
    pub state: u8,
}

impl ProposalStatus {
    pub fn new(proposal_id: u64, address: H160, state: ProposalState) -> Self {
        Self {
            proposal_id,
            address: format!("{:?}", address),
            state: state as u8,
        }
    }
}
pub struct XProtocol;

impl XProtocol {
    pub fn get_index<T>(array: &[T], i: usize) -> BoxedSyncResult<T>
    where
        T: Clone,
    {
        array
            .get(i)
            .cloned()
            .ok_or_else(|| format!("{} index out of bound", i).into())
    }

    pub fn parse_log(
        raw_log: &ethabi::RawLog,
        contract: &ethabi::Contract,
        name: &str,
    ) -> BoxedSyncResult<ethabi::Log> {
        let event = contract.event(name)?;
        let abi_event: ethabi::Event = event.clone();
        abi_event.parse_log(raw_log.clone()).map_err(|e| e.into())
    }
}

pub trait ModuleTest {
    fn with_fn<'a>(&'a self, log: &'a Log) -> LocalBoxFuture<'a, BoxedSyncResult<()>>;
}
