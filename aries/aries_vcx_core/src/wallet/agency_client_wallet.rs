use std::{collections::HashMap, sync::Arc};

use agency_client::{
    errors::error::{AgencyClientError, AgencyClientErrorKind, AgencyClientResult},
    wallet::base_agency_client_wallet::BaseAgencyClientWallet,
};
use async_trait::async_trait;
#[cfg(feature = "vdrtools_wallet")]
use vdrtools::WalletHandle;

use super::structs_io::UnpackMessageOutput;
use crate::{
    errors::error::{AriesVcxCoreError, AriesVcxCoreErrorKind, VcxCoreResult},
    utils::async_fn_iterator::AsyncFnIterator,
    wallet::base_wallet::BaseWallet,
};

#[derive(Debug)]
pub struct AgencyClientWallet {
    inner: Arc<dyn BaseAgencyClientWallet>,
}

/// Implementation of [BaseWallet] for [AgencyClientWallet] such that a [BaseAgencyClientWallet]
/// can be converted to a [BaseWallet] for packing and unpacking messages, and vice versa.
#[allow(unused_variables)]
#[async_trait]
impl BaseWallet for AgencyClientWallet {
    async fn create_and_store_my_did(
        &self,
        seed: Option<&str>,
        method_name: Option<&str>,
    ) -> VcxCoreResult<(String, String)> {
        Err(unimplemented_agency_client_wallet_method(
            "create_and_store_my_did",
        ))
    }

    async fn key_for_local_did(&self, did: &str) -> VcxCoreResult<String> {
        Err(unimplemented_agency_client_wallet_method(
            "get_verkey_from_wallet",
        ))
    }

    async fn replace_did_keys_start(&self, target_did: &str) -> VcxCoreResult<String> {
        Err(unimplemented_agency_client_wallet_method(
            "replace_did_keys_start",
        ))
    }

    async fn replace_did_keys_apply(&self, target_did: &str) -> VcxCoreResult<()> {
        Err(unimplemented_agency_client_wallet_method(
            "replace_did_key_apply",
        ))
    }

    async fn add_wallet_record(
        &self,
        xtype: &str,
        id: &str,
        value: &str,
        tags: Option<HashMap<String, String>>,
    ) -> VcxCoreResult<()> {
        Err(unimplemented_agency_client_wallet_method(
            "add_wallet_record",
        ))
    }

    async fn get_wallet_record(
        &self,
        xtype: &str,
        id: &str,
        options: &str,
    ) -> VcxCoreResult<String> {
        Err(unimplemented_agency_client_wallet_method(
            "get_wallet_record",
        ))
    }

    async fn get_wallet_record_value(&self, xtype: &str, id: &str) -> VcxCoreResult<String> {
        Err(unimplemented_agency_client_wallet_method(
            "get_wallet_record_value",
        ))
    }

    async fn delete_wallet_record(&self, xtype: &str, id: &str) -> VcxCoreResult<()> {
        Err(unimplemented_agency_client_wallet_method(
            "delete_wallet_record",
        ))
    }

    async fn update_wallet_record_value(
        &self,
        xtype: &str,
        id: &str,
        value: &str,
    ) -> VcxCoreResult<()> {
        Err(unimplemented_agency_client_wallet_method(
            "update_wallet_record_value",
        ))
    }

    async fn add_wallet_record_tags(
        &self,
        xtype: &str,
        id: &str,
        tags: HashMap<String, String>,
    ) -> VcxCoreResult<()> {
        Err(unimplemented_agency_client_wallet_method(
            "add_wallet_record_tags",
        ))
    }

    async fn delete_wallet_record_tags(
        &self,
        xtype: &str,
        id: &str,
        tag_names: &str,
    ) -> VcxCoreResult<()> {
        Err(unimplemented_agency_client_wallet_method(
            "delete_wallet_record_tags",
        ))
    }

    async fn update_wallet_record_tags(
        &self,
        xtype: &str,
        id: &str,
        tags: HashMap<String, String>,
    ) -> VcxCoreResult<()> {
        Err(unimplemented_agency_client_wallet_method(
            "update_wallet_record_tags",
        ))
    }

    async fn iterate_wallet_records(
        &self,
        xtype: &str,
        query: &str,
        options: &str,
    ) -> VcxCoreResult<Box<dyn AsyncFnIterator<Item = VcxCoreResult<String>>>> {
        Err(unimplemented_agency_client_wallet_method(
            "iterate_wallet_records",
        ))
    }

    async fn sign(&self, my_vk: &str, msg: &[u8]) -> VcxCoreResult<Vec<u8>> {
        Err(unimplemented_agency_client_wallet_method("sign"))
    }

    async fn verify(&self, vk: &str, msg: &[u8], signature: &[u8]) -> VcxCoreResult<bool> {
        Err(unimplemented_agency_client_wallet_method("verify"))
    }

    async fn pack_message(
        &self,
        sender_vk: Option<&str>,
        receiver_keys: &str,
        msg: &[u8],
    ) -> VcxCoreResult<Vec<u8>> {
        Ok(self
            .inner
            .pack_message(sender_vk, receiver_keys, msg)
            .await?)
    }

    async fn unpack_message(&self, msg: &[u8]) -> VcxCoreResult<UnpackMessageOutput> {
        let unpack_json_bytes = self.inner.unpack_message(msg).await?;
        serde_json::from_slice(&unpack_json_bytes[..]).map_err(|err| {
            AriesVcxCoreError::from_msg(AriesVcxCoreErrorKind::ParsingError, err.to_string())
        })
    }

    #[cfg(feature = "vdrtools_wallet")]
    fn get_wallet_handle(&self) -> WalletHandle {
        unimplemented!("AgencyClientWallet::get_wallet_handle - this was not expected to be called")
    }
}

pub trait ToBaseWallet {
    fn to_base_wallet(&self) -> AgencyClientWallet;
}

impl ToBaseWallet for Arc<dyn BaseAgencyClientWallet> {
    fn to_base_wallet(&self) -> AgencyClientWallet {
        AgencyClientWallet {
            inner: Arc::clone(self),
        }
    }
}

fn unimplemented_agency_client_wallet_method(method_name: &str) -> AriesVcxCoreError {
    // should not occur with proper internal usage - [AgencyClientWallet] is not public
    AriesVcxCoreError::from_msg(
        AriesVcxCoreErrorKind::UnimplementedFeature,
        format!("AgencyClientWallet::{method_name} is not intended to be used."),
    )
}

#[derive(Debug)]
pub(crate) struct BaseWalletAgencyClientWallet {
    inner: Arc<dyn BaseWallet>,
}

/// Implementation of [BaseAgencyClientWallet] which wraps over an [BaseWallet] implementation
/// to allow conversion
#[async_trait]
impl BaseAgencyClientWallet for BaseWalletAgencyClientWallet {
    async fn pack_message(
        &self,
        sender_vk: Option<&str>,
        receiver_keys: &str,
        msg: &[u8],
    ) -> AgencyClientResult<Vec<u8>> {
        self.inner
            .pack_message(sender_vk, receiver_keys, msg)
            .await
            .map_err(|e| {
                AgencyClientError::from_msg(
                    AgencyClientErrorKind::UnknownError,
                    format!("A VCXError occured while calling pack_message: {e:?}"),
                )
            })
    }

    async fn unpack_message(&self, msg: &[u8]) -> AgencyClientResult<Vec<u8>> {
        let unpack = self.inner.unpack_message(msg).await.map_err(|e| {
            AgencyClientError::from_msg(
                AgencyClientErrorKind::UnknownError,
                format!("A VCXError occured while calling unpack_message: {e:?}"),
            )
        })?;
        serde_json::to_vec(&unpack).map_err(|err| {
            AgencyClientError::from_msg(
                AgencyClientErrorKind::UnknownError,
                format!("A VCXError occured while calling unpack_message: {err:?}"),
            )
        })
    }
}

pub trait ToBaseAgencyClientWallet {
    fn to_base_agency_client_wallet(&self) -> Arc<dyn BaseAgencyClientWallet>;
}

impl<T> ToBaseAgencyClientWallet for Arc<T>
where
    T: BaseWallet + 'static,
{
    fn to_base_agency_client_wallet(&self) -> Arc<dyn BaseAgencyClientWallet> {
        let x = self.clone();
        Arc::new(BaseWalletAgencyClientWallet { inner: x })
    }
}
